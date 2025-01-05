use core::slice;
use std::{ffi::CString, fs::File, io::Write, path::Path};

use crate::db::internal::error::{PPError, PPResult};

use ffmpeg_sys_next::{
    av_frame_alloc, av_frame_free, av_free, av_image_fill_arrays, av_image_get_buffer_size,
    av_malloc, av_packet_alloc, av_packet_free, av_packet_unref, av_read_frame,
    avcodec_alloc_context3, avcodec_find_decoder, avcodec_find_encoder, avcodec_free_context,
    avcodec_open2, avcodec_parameters_to_context, avcodec_receive_frame, avcodec_receive_packet,
    avcodec_send_frame, avcodec_send_packet, avdevice_register_all, avformat_close_input,
    avformat_find_stream_info, avformat_open_input, sws_freeContext, sws_getContext, sws_scale,
    AVCodec, AVCodecContext, AVCodecID, AVCodecParameters, AVColorRange, AVFormatContext, AVFrame,
    AVMediaType, AVPacket, AVPixelFormat, AVRational, SwsContext, SWS_BILINEAR,
};
use image::{imageops::FilterType, ImageBuffer, RgbImage};

pub enum ThumbnailQuality {
    Good,
    Medium,
    Bad,
}

#[allow(unused_assignments)]
pub fn generate_thumbnail(
    input_path: &str,
    output_path: impl AsRef<Path>,
    quality: ThumbnailQuality,
) -> PPResult<()> {
    let mut format_ctx: *mut AVFormatContext = std::ptr::null_mut();
    let mut codec_ctx: *mut AVCodecContext = std::ptr::null_mut();
    let mut codec: *const AVCodec = std::ptr::null_mut();
    let mut frame: *mut AVFrame = std::ptr::null_mut();
    let mut rgb_frame: *mut AVFrame = std::ptr::null_mut();

    let mut packet: *mut AVPacket = std::ptr::null_mut();
    let mut sws_context: *mut SwsContext = std::ptr::null_mut();
    let mut video_stream_idx: isize = -1;

    // Register all devices
    unsafe { avdevice_register_all() }

    // Open input file
    let input_cstr =
        CString::new(input_path).map_err(|_| PPError::from("Failed to create CString"))?;
    if unsafe {
        avformat_open_input(
            &mut format_ctx,
            input_cstr.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } < 0
    {
        return Err("Failed to open format".into());
    }

    // Retrieve stream information
    if unsafe { avformat_find_stream_info(format_ctx, std::ptr::null_mut()) } < 0 {
        println!();
        unsafe { avformat_close_input(&mut format_ctx) };
        return Err("Couldn't find the stream info".into());
    }

    // Find video stream
    for i in 0..unsafe { (*format_ctx).nb_streams } {
        if unsafe { (*(*(*(*format_ctx).streams.offset(i as isize))).codecpar).codec_type }
            == AVMediaType::AVMEDIA_TYPE_VIDEO
        {
            video_stream_idx = i as isize;
            break;
        }
    }

    if video_stream_idx == -1 {
        unsafe { avformat_close_input(&mut format_ctx) };
        return Err("Couldn't find the video stream index".into());
    }

    // Get codec parameters and find decoder
    let codec_params: *mut AVCodecParameters =
        unsafe { (**(*format_ctx).streams.offset(video_stream_idx)).codecpar };
    codec = unsafe { avcodec_find_decoder((*codec_params).codec_id) };

    if codec.is_null() {
        unsafe { avformat_close_input(&mut format_ctx) };
        return Err("Unsupported codec".into());
    }

    codec_ctx = unsafe { avcodec_alloc_context3(codec) };
    if codec_ctx.is_null() {
        println!("Failed to allocate codec context");
        unsafe { avformat_close_input(&mut format_ctx) };
        return Ok(());
    }

    if unsafe { avcodec_parameters_to_context(codec_ctx, codec_params) } < 0 {
        println!();
        unsafe {
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut format_ctx);
        }
        return Err("Could not copy codec parameters to codec context".into());
    }

    if unsafe { avcodec_open2(codec_ctx, codec, std::ptr::null_mut()) } < 0 {
        unsafe {
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut format_ctx);
        }
        return Err("Couldn't open codec".into());
    }

    // Allocate frames and packet
    frame = unsafe { av_frame_alloc() };
    rgb_frame = unsafe { av_frame_alloc() };
    packet = unsafe { av_packet_alloc() };

    if frame.is_null() || frame.is_null() || packet.is_null() {
        println!();
        unsafe {
            av_frame_free(&mut frame);
            av_frame_free(&mut rgb_frame);
            av_packet_free(&mut packet);
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut format_ctx);
        }
        return Err("Failed to allocate frame or packet".into());
    }

    // Set up sws context for frame conversion
    let width: i32 = unsafe { (*codec_ctx).width };
    let height: i32 = unsafe { (*codec_ctx).height };
    sws_context = unsafe {
        sws_getContext(
            width,
            height,
            (*codec_ctx).pix_fmt,
            width,
            height,
            AVPixelFormat::AV_PIX_FMT_RGB24,
            SWS_BILINEAR,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };

    if sws_context.is_null() {
        unsafe {
            av_frame_free(&mut frame);
            av_frame_free(&mut rgb_frame);
            av_packet_free(&mut packet);
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut format_ctx);
        }
        return Err("Failed to create SWS context".into());
    }

    let num_bytes =
        unsafe { av_image_get_buffer_size(AVPixelFormat::AV_PIX_FMT_RGB24, width, height, 1) };
    let buffer = unsafe { av_malloc(num_bytes as usize) as *mut u8 };

    if buffer.is_null() {
        unsafe {
            sws_freeContext(sws_context);
            av_frame_free(&mut frame);
            av_frame_free(&mut rgb_frame);
            av_packet_free(&mut packet);
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut format_ctx);
        }
        return Err("Failed to allocate buffer".into());
    }

    unsafe {
        av_image_fill_arrays(
            (*rgb_frame).data.as_mut_ptr(),
            (*rgb_frame).linesize.as_mut_ptr(),
            buffer,
            AVPixelFormat::AV_PIX_FMT_RGB24,
            width,
            height,
            1,
        );
    }

    unsafe {
        (*frame).color_range = AVColorRange::AVCOL_RANGE_MPEG;
    }

    while unsafe { av_read_frame(format_ctx, packet) } >= 0 {
        if unsafe { (*packet).stream_index as isize == video_stream_idx }
            && unsafe { avcodec_send_packet(codec_ctx, packet) } >= 0
            && unsafe { avcodec_receive_frame(codec_ctx, frame) } >= 0
        {
            // Convert to RGB
            unsafe {
                sws_scale(
                    sws_context,
                    (*frame).data.as_ptr() as *const *const u8,
                    (*frame).linesize.as_ptr(),
                    0,
                    height,
                    (*rgb_frame).data.as_mut_ptr(),
                    (*rgb_frame).linesize.as_mut_ptr(),
                );
            }

            // Save the frame as a JPEG (or any other format you prefer)
            save_thumbnail(rgb_frame, width, height, output_path, quality)?;

            break;
        }
        unsafe { av_packet_unref(packet) };
    }

    // Free resources
    unsafe {
        av_free(buffer as *mut _);
        sws_freeContext(sws_context);
        av_frame_free(&mut frame);
        av_frame_free(&mut rgb_frame);
        av_packet_free(&mut packet);
        avcodec_free_context(&mut codec_ctx);
        avformat_close_input(&mut format_ctx);
    }
    Ok(())
}

fn save_thumbnail(
    frame: *mut AVFrame,
    width: i32,
    height: i32,
    output_path: impl AsRef<Path>,
    quality: ThumbnailQuality,
) -> PPResult<()> {
    let buffer =
        unsafe { std::slice::from_raw_parts((*frame).data[0], (width * height * 3) as usize) };

    let input_image = RgbImage::from_raw(width as u32, height as u32, buffer.to_vec()).ok_or(
        std::io::Error::new(std::io::ErrorKind::Other, "Invalid buffer size"),
    )?;

    // Calculate new dimensions
    let (new_width, new_height) = match quality {
        ThumbnailQuality::Good => (width, height),
        ThumbnailQuality::Medium => (width / 2, height / 2),
        ThumbnailQuality::Bad => (width / 4, height / 4),
    };

    // Resize the image
    let resized_image = image::imageops::resize(
        &input_image,
        new_width as u32,
        new_height as u32,
        FilterType::CatmullRom,
    );

    let res = std::panic::catch_unwind(|| -> std::io::Result<Vec<u8>> {
        let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);

        comp.set_size(
            new_width.try_into().unwrap(),
            new_height.try_into().unwrap(),
        );
        match quality {
            ThumbnailQuality::Good => {
                comp.set_quality(60.0);
            }
            ThumbnailQuality::Medium => {
                comp.set_quality(40.0);
            }
            ThumbnailQuality::Bad => {
                comp.set_quality(20.0);
            }
        }
        let mut comp = comp.start_compress(Vec::new())?;

        comp.write_scanlines(&resized_image)?;

        let writer = comp.finish()?;
        Ok(writer)
    });

    match res {
        Ok(Ok(jpeg_data)) => {
            // Save the JPEG directly to the output path
            std::fs::write(output_path, jpeg_data)?;
        }
        Ok(Err(io_err)) => {
            eprintln!("I/O error during JPEG compression: {:?}", io_err);
            return Err(io_err.into());
        }
        Err(panic_err) => {
            eprintln!("Panic during thumbnail generation: {:?}", panic_err);
            return Err("Panic occurred during thumbnail generation".into());
        }
    };

    Ok(())
}
