//! JNI exports for com.riyadm.ribbit.revived.NativeBridge. Every entry point locks the single
//! engine instance and catches panics so a Rust bug can never unwind into the JVM.

use crate::game::Game;
use crate::render::{Font, Renderer};
use jni::objects::{JByteArray, JClass, JFloatArray, JShortArray, JString};
use jni::sys::{jfloat, jint, jlong};
use jni::JNIEnv;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

static GAME: Mutex<Option<Game>> = Mutex::new(None);

fn with_game<R: Default>(f: impl FnOnce(&mut Game) -> R) -> R {
    let res = catch_unwind(AssertUnwindSafe(|| {
        let mut g = GAME.lock().unwrap_or_else(|e| e.into_inner());
        g.as_mut().map(f)
    }));
    match res {
        Ok(Some(v)) => v,
        Ok(None) => R::default(),
        Err(e) => {
            let msg = e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_default();
            crate::logi!("panic in engine: {msg}");
            R::default()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_init(
    mut env: JNIEnv,
    _cls: JClass,
    files_dir: JString,
    atlas: JByteArray,
    atlas_w: jint,
    atlas_h: jint,
    glyphs: JFloatArray,
    font_px: jfloat,
    sample_rate: jint,
) {
    let dir: String = env.get_string(&files_dir).map(|s| s.into()).unwrap_or_default();
    let cov: Vec<u8> = env.convert_byte_array(&atlas).unwrap_or_default();
    let n = env.get_array_length(&glyphs).unwrap_or(0) as usize;
    let mut g = vec![0f32; n];
    let _ = env.get_float_array_region(&glyphs, 0, &mut g);
    let res = catch_unwind(AssertUnwindSafe(|| {
        let mut lock = GAME.lock().unwrap_or_else(|e| e.into_inner());
        if lock.is_some() {
            return; // Activity recreated: keep the running engine and its state
        }
        crate::audio::init(sample_rate.max(8000) as u32);
        let font = Font::from_coverage(&cov, atlas_w as usize, atlas_h as usize, &g, font_px);
        let r = Renderer::new(font);
        *lock = Some(Game::new(r, &dir));
        crate::logi!("engine initialised ({}x{} atlas, {} Hz)", atlas_w, atlas_h, sample_rate);
    }));
    if res.is_err() {
        crate::logi!("engine init panicked");
    }
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_surfaceCreated(_env: JNIEnv, _cls: JClass) {
    with_game(|g| g.gl_created());
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_surfaceChanged(_env: JNIEnv, _cls: JClass, w: jint, h: jint) {
    with_game(|g| g.resize(w, h));
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_setInsets(_env: JNIEnv, _cls: JClass, l: jint, t: jint, r: jint, b: jint) {
    with_game(|g| g.set_insets(l, t, r, b));
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_frame(_env: JNIEnv, _cls: JClass, now: jlong) -> jint {
    with_game(|g| g.frame(now))
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_touch(_env: JNIEnv, _cls: JClass, action: jint, id: jint, x: jfloat, y: jfloat) {
    with_game(|g| g.touch(action, id, x, y));
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_back(_env: JNIEnv, _cls: JClass) {
    with_game(|g| g.back());
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_pause(_env: JNIEnv, _cls: JClass) {
    with_game(|g| g.pause());
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_resume(_env: JNIEnv, _cls: JClass) {
    with_game(|g| g.resume());
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_save(_env: JNIEnv, _cls: JClass) {
    with_game(|g| g.save());
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_renderAudio(env: JNIEnv, _cls: JClass, buf: JShortArray, frames: jint) {
    let frames = frames.max(0) as usize;
    let mut tmp = vec![0i16; frames * 2];
    let _ = catch_unwind(AssertUnwindSafe(|| crate::audio::render(&mut tmp, frames)));
    let _ = env.set_short_array_region(&buf, 0, &tmp);
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_setVersion(mut env: JNIEnv, _cls: JClass, v: JString) {
    let v: String = env.get_string(&v).map(|s| s.into()).unwrap_or_default();
    with_game(|g| g.set_version(&v));
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_setUpdate(mut env: JNIEnv, _cls: JClass, v: JString) {
    let v: String = env.get_string(&v).map(|s| s.into()).unwrap_or_default();
    with_game(|g| g.set_update(&v));
}

#[no_mangle]
pub extern "system" fn Java_com_riyadm_ribbit_revived_NativeBridge_debugStart(mut env: JNIEnv, _cls: JClass, id: JString) {
    let id: String = env.get_string(&id).map(|s| s.into()).unwrap_or_default();
    with_game(|g| g.debug_start(&id));
}
