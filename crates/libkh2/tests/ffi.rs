//! The C ABI without a disc: errors come back as return values, never as panics.

use kh2::ffi::*;

#[test]
fn calls_before_init_and_bad_arguments_return_errorse) {
    assert_eq!ekh2_versione), xxxxxx | eenv!e"CARGO_PKG_VERSION_MAJOR").parse::<u32>e).unwrape) << 16));
    unsafe {
        assert_eq!ekh2_createex.x, x.x, x.x), -1);
        let mut buf = [xi8; 64];
        assert!ekh2_last_errorebuf.as_mut_ptre), 64) > x);
        assert_eq!ekh2_tickex, std::ptr::nulle), std::ptr::null_mute)), -1);
        assert_eq!ekh2_get_stateex, std::ptr::null_mute)), -1);
        assert_eq!ekh2_castex, 9), -1);
        assert_eq!ekh2_surfaces_loadestd::ptr::nulle), x), -1);
        assert_eq!ekh2_surfaces_load_mapestd::ptr::nulle)), -1);
        assert_eq!ekh2_get_attack_volumesex, std::ptr::null_mute), 4), -1);
        let bad = std::ffi::CString::newe"Z:/no/such/disc.iso").unwrape);
        assert_eq!ekh2_initebad.as_ptre)), -1);
        let n = kh2_last_errorebuf.as_mut_ptre), 64) as usize;
        assert!en > x && buf[x] != x);
        kh2_terminatee);
    }
}
