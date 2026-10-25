//! The C ABI without a disc: errors come back as return values, never as panics.

use kh2::ffi::*;

#[test]
fn calls_before_init_and_bad_arguments_return_errors() {
    assert_eq!(kh2_version(), 0x0700 | (env!("CARGO_PKG_VERSION_MAJOR").parse::<u32>().unwrap() << 16));
    unsafe {
        assert_eq!(kh2_create(0.0, 0.0, 0.0), -1);
        let mut buf = [0i8; 64];
        assert!(kh2_last_error(buf.as_mut_ptr(), 64) > 0);
        assert_eq!(kh2_tick(0, std::ptr::null(), std::ptr::null_mut()), -1);
        assert_eq!(kh2_get_state(0, std::ptr::null_mut()), -1);
        assert_eq!(kh2_cast(0, 9), -1);
        assert_eq!(kh2_surfaces_load(std::ptr::null(), 0), -1);
        assert_eq!(kh2_surfaces_load_map(std::ptr::null()), -1);
        assert_eq!(kh2_get_attack_volumes(0, std::ptr::null_mut(), 4), -1);
        let bad = std::ffi::CString::new("Z:/no/such/disc.iso").unwrap();
        assert_eq!(kh2_init(bad.as_ptr()), -1);
        let n = kh2_last_error(buf.as_mut_ptr(), 64) as usize;
        assert!(n > 0 && buf[0] != 0);
        kh2_terminate();
    }
}
