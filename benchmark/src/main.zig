const std = @import("std");

fn increment(integer: *u16) void {
    integer.* += 1;
}

pub fn main() !void {
    const str: *[]u8 = "Hello";
    const integer = 5;
    increment(&integer);
    std.debug.print("{}", .{str});
}
