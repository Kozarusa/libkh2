//! Moving collision objects through the C API (round 10, like libsm64's surface objects): a
//! platform the host moves carries Sora standing on it, a rotating one turns him with it, and a
//! deleted one leaves the floor below. Needs `KH2_ISO` (Sora's data); skipped otherwise.

use std::ffi::CString;

use kh2::ffi::*;
use kh2::Surface;

fn quad(x0: f32, x1: f32, z0: f32, z1: f32, y: f32) -> [Surface; 2] {
    let t = |v| Surface { v, flags: 1, ..Default::default() };
    [t([[x0, y, z0], [x0, y, z1], [x1, y, z1]]), t([[x0, y, z0], [x1, y, z1], [x1, y, z0]])]
}

fn state(h: i32) -> Kh2State {
    let mut s = Kh2State::default();
    unsafe { kh2_get_state(h, &mut s) };
    s
}

#[test]
fn a_moving_platform_carries_sora() {
    let Ok(iso) = std::env::var("KH2_ISO_FM").or_else(|_| std::env::var("KH2_ISO")) else { return eprintln!("skipped: no KH2_ISO") };
    let iso = CString::new(iso).unwrap();
    assert_eq!(unsafe { kh2_init(iso.as_ptr()) }, 0);
    // A floor far below and a 400 x 400 platform (its own space: top at y 0) at height 200.
    let floor = quad(-5000.0, 5000.0, -5000.0, 5000.0, -1000.0);
    assert!(unsafe { kh2_surfaces_load(floor.as_ptr(), 2) } > 0);
    let top = quad(-200.0, 200.0, -200.0, 200.0, 0.0);
    let mut tf = Kh2ObjectTransform { pos: [0.0, 200.0, 0.0], rot: [0.0; 3] };
    let id = unsafe { kh2_surface_object_create(top.as_ptr(), 2, &tf) };
    assert!(id > 0);
    let h = kh2_create(0.0, 200.0, 0.0);
    let idle = Kh2Inputs { cam_look_z: 1.0, ..Default::default() };
    let mut st = Kh2State::default();
    for _ in 0..30 {
        unsafe { kh2_tick(h, &idle, &mut st) };
    }
    assert_eq!(st.grounded, 1);
    assert!((st.pos[1] - 200.0).abs() < 2.0, "stands on the platform: {:?}", st.pos);
    // The platform slides 2 cm a tick along x for 2 s.
    for _ in 0..120 {
        tf.pos[0] += 2.0;
        unsafe { kh2_surface_object_move(id, &tf) };
        unsafe { kh2_tick(h, &idle, &mut st) };
    }
    eprintln!("after the slide: {:?}", st.pos);
    assert!((st.pos[0] - 240.0).abs() < 5.0 && (st.pos[1] - 200.0).abs() < 2.0, "carried: {:?}", st.pos);
    // It turns a quarter round about its centre in 90 ticks: Sora at its centre turns with it.
    let yaw0 = state(h).yaw;
    let centre = tf.pos;
    for _ in 0..90 {
        tf.rot[1] += std::f32::consts::FRAC_PI_2 / 90.0;
        unsafe { kh2_surface_object_move(id, &tf) };
        unsafe { kh2_tick(h, &idle, &mut st) };
    }
    let turned = kh2_game_core::motor::fix_radian(st.yaw - yaw0);
    eprintln!("turned {turned:.3} rad, at {:?} (centre {centre:?})", st.pos);
    assert!((turned - std::f32::consts::FRAC_PI_2).abs() < 0.05, "turns with it");
    // Deleted: he falls to the floor below.
    assert_eq!(kh2_surface_object_delete(id), 0);
    for _ in 0..240 {
        unsafe { kh2_tick(h, &idle, &mut st) };
    }
    assert!(st.pos[1] < -900.0, "fell to the floor: {:?}", st.pos);
    kh2_delete(h);
    kh2_terminate();
}
