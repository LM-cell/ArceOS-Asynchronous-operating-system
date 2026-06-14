use std::hint::black_box;

/// Touches stack pages recursively so RSS can reflect committed stack pages.
///
/// A configured stack size is often only virtual address reservation. This
/// helper creates controlled stack pressure before each task reaches sleep.
pub fn touch_stack_bytes(bytes: usize) -> u64 {
    const PAGE: usize = 4096;

    fn recurse(remaining: usize, checksum: u64) -> u64 {
        let frame = [0xA5_u8; PAGE];
        let first = unsafe { std::ptr::read_volatile(frame.as_ptr()) as u64 };
        let last = unsafe { std::ptr::read_volatile(frame.as_ptr().add(PAGE - 1)) as u64 };
        let checksum = checksum.wrapping_add(first).wrapping_add(last);

        let nested = if remaining > PAGE {
            recurse(remaining - PAGE, checksum)
        } else {
            checksum
        };

        // Keep this frame alive after the recursive call to discourage tail-call
        // style optimization from reusing a single frame.
        black_box(&frame);
        nested
    }

    if bytes == 0 {
        return 0;
    }

    black_box(recurse(bytes, 0))
}
