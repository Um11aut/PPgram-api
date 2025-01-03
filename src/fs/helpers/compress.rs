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

pub enum ThumbnailQuality {
    Good,
    Medium,
    Bad,
}

#[allow(unused_assignments)]
pub fn generate_thumbnail(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    quality: ThumbnailQuality,
) -> PPResult<()> {
    let mut format_ctx: *mut AVFormatContext = std::ptr::null_mut();
    let mut codec_ctx: *mut AVCodecContext = std::ptr::null_mut();
    let mut codec: *const AVCodec = std::ptr::null_mut();
    let mut frame: *mut AVFrame = std::ptr::null_mut();
    let mut packet: *mut AVPacket = std::ptr::null_mut();
    let mut sws_context: *mut SwsContext = std::ptr::null_mut();
    let mut video_stream_idx: isize = -1;

    // Register all devices
    unsafe { avdevice_register_all() }

    // Open input file
    let input_cstr = CString::new(input_path.as_ref().to_str().unwrap().as_bytes().to_vec())
        .map_err(|err| PPError::from(err.to_string()))?;
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
    packet = unsafe { av_packet_alloc() };

    if frame.is_null() || frame.is_null() || packet.is_null() {
        println!();
        unsafe {
            av_frame_free(&mut frame);
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
            av_packet_free(&mut packet);
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut format_ctx);
        }
        return Err("Failed to allocate buffer".into());
    }

    unsafe {
        av_image_fill_arrays(
            (*frame).data.as_mut_ptr(),
            (*frame).linesize.as_mut_ptr(),
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
                    (*frame).data.as_mut_ptr(),
                    (*frame).linesize.as_mut_ptr(),
                );
            }

            // Save the frame as a JPEG (or any other format you prefer)
            save_thumbnail(frame, width, height, output_path.as_ref(), quality)?;

            break;
        }
        unsafe { av_packet_unref(packet) };
    }

    // Free resources
    unsafe {
        av_free(buffer as *mut _);
        sws_freeContext(sws_context);
        av_frame_free(&mut frame);
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
    unsafe {
        // Find JPEG codec
        let codec = avcodec_find_encoder(AVCodecID::AV_CODEC_ID_MJPEG);
        if codec.is_null() {
            return Err("JPEG codec not found".into());
        }

        // Allocate codec context for JPEG encoder
        let mut codec_ctx = avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            return Err("Failed to allocate codec context".into());
        }

        (*codec_ctx).width = width;
        (*codec_ctx).height = height;
        (*codec_ctx).pix_fmt = AVPixelFormat::AV_PIX_FMT_YUVJ420P; // Set the pixel format to YUVJ420p
        (*codec_ctx).time_base = AVRational { num: 1, den: 25 };
        match quality {
            ThumbnailQuality::Good => {
                (*codec_ctx).qcompress = 0.8;
                (*codec_ctx).qmax = 32;
                (*codec_ctx).qmin = 20;
            }
            ThumbnailQuality::Medium => {
                (*codec_ctx).qcompress = 0.5;
                (*codec_ctx).qmax = 32;
                (*codec_ctx).qmin = 25;
            },
            ThumbnailQuality::Bad => {
                (*codec_ctx).qcompress = 0.1;
                (*codec_ctx).qmax = 32;
                (*codec_ctx).qmin = 30;
            },
        }

        if avcodec_open2(codec_ctx, codec, std::ptr::null_mut()) < 0 {
            avcodec_free_context(&mut codec_ctx);
            return Err("Failed to open codec".into());
        }

        // Allocate a new frame for YUVJ420p format
        let mut yuv_frame = av_frame_alloc();
        if yuv_frame.is_null() {
            avcodec_free_context(&mut codec_ctx);
            return Err("Failed to allocate YUV frame".into());
        }

        // Set up the frame to hold YUVJ420p data
        (*yuv_frame).format = AVPixelFormat::AV_PIX_FMT_YUVJ420P as i32;
        (*yuv_frame).width = width;
        (*yuv_frame).height = height;

        // Allocate memory for the YUV frame
        let buffer_size =
            av_image_get_buffer_size(AVPixelFormat::AV_PIX_FMT_YUVJ420P, width, height, 1);
        let buffer = av_malloc(buffer_size as usize);
        if buffer.is_null() {
            av_frame_free(&mut yuv_frame);
            avcodec_free_context(&mut codec_ctx);
            return Err("Failed to allocate buffer for YUV frame".into());
        }

        // Fill the YUV frame with data
        av_image_fill_arrays(
            (*yuv_frame).data.as_mut_ptr(),
            (*yuv_frame).linesize.as_mut_ptr(),
            buffer as *mut u8,
            AVPixelFormat::AV_PIX_FMT_YUVJ420P,
            width,
            height,
            1,
        );

        // Create SWS context to convert RGB frame to YUVJ420p
        #[allow(unused_assignments)]
        let mut sws_context: *mut SwsContext = std::ptr::null_mut();
        sws_context = sws_getContext(
            width,
            height,
            AVPixelFormat::AV_PIX_FMT_RGB24, // Input format is RGB24
            width,
            height,
            AVPixelFormat::AV_PIX_FMT_YUVJ420P, // Output format is YUVJ420p
            SWS_BILINEAR,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        if sws_context.is_null() {
            av_free(buffer);
            av_frame_free(&mut yuv_frame);
            avcodec_free_context(&mut codec_ctx);
            return Err("Failed to create SWS context".into());
        }

        // Convert the frame from RGB to YUVJ420p
        sws_scale(
            sws_context,
            (*frame).data.as_ptr() as *const *const u8,
            (*frame).linesize.as_ptr(),
            0,
            height,
            (*yuv_frame).data.as_mut_ptr(),
            (*yuv_frame).linesize.as_mut_ptr(),
        );

        // Free the SWS context
        sws_freeContext(sws_context);

        // Now send the YUVJ420p frame to the JPEG encoder
        let mut packet = av_packet_alloc();
        if packet.is_null() {
            av_free(buffer);
            av_frame_free(&mut yuv_frame);
            avcodec_free_context(&mut codec_ctx);
            return Err("Failed to allocate packet".into());
        }

        if avcodec_send_frame(codec_ctx, yuv_frame) < 0 {
            av_packet_free(&mut packet);
            av_free(buffer);
            av_frame_free(&mut yuv_frame);
            avcodec_free_context(&mut codec_ctx);
            return Err("Failed to send frame to encoder".into());
        }

        if avcodec_receive_packet(codec_ctx, packet) < 0 {
            av_packet_free(&mut packet);
            av_free(buffer);
            av_frame_free(&mut yuv_frame);
            avcodec_free_context(&mut codec_ctx);
            return Err("Failed to receive packet from encoder".into());
        }

        // Write the packet data to a file
        let mut file = File::create(output_path)?;
        file.write_all(std::slice::from_raw_parts(
            (*packet).data,
            (*packet).size as usize,
        ))?;

        // Free resources
        av_packet_free(&mut packet);
        av_free(buffer);
        av_frame_free(&mut yuv_frame);
        avcodec_free_context(&mut codec_ctx);
    }

    Ok(())
}
