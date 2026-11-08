package com.riyadm.ribbit.revived

/**
 * JNI surface of the Rust engine (rust/src/jni_api.rs). All game logic, rendering and audio
 * synthesis live in libribbit.so; this layer only forwards platform events.
 */
object NativeBridge {
    init {
        System.loadLibrary("ribbit")
    }

    /** Flags returned by [frame]. Keep in sync with rust/src/platform.rs. */
    const val REQ_EXIT = 1
    const val REQ_HAPTIC_TICK = 2
    const val REQ_HAPTIC_CONFIRM = 4
    const val REQ_HAPTIC_REJECT = 8
    const val REQ_HAPTIC_HEAVY = 16
    const val REQ_SAVE = 32
    const val REQ_CHECK_UPDATE = 64
    const val REQ_OPEN_UPDATE = 128

    const val TOUCH_DOWN = 0
    const val TOUCH_MOVE = 1
    const val TOUCH_UP = 2
    const val TOUCH_CANCEL = 3

    /**
     * One-time engine setup. [glyphs] holds 7 floats per character (code 32..126):
     * atlasX, atlasY, width, height, bearingX, bearingY (top relative to baseline), advance.
     */
    external fun init(
        filesDir: String,
        atlas: ByteArray,
        atlasWidth: Int,
        atlasHeight: Int,
        glyphs: FloatArray,
        fontPx: Float,
        sampleRate: Int,
    )

    external fun surfaceCreated()
    external fun surfaceChanged(width: Int, height: Int)
    external fun setInsets(left: Int, top: Int, right: Int, bottom: Int)
    external fun frame(nowNanos: Long): Int
    external fun touch(action: Int, pointerId: Int, x: Float, y: Float)
    external fun back()
    external fun pause()
    external fun resume()
    external fun save()

    /** Installed versionName, shown in Options. */
    external fun setVersion(version: String)

    /** A newer release found on GitHub (empty string clears it); shows the menu's update button. */
    external fun setUpdate(version: String)

    /** Debug builds only: jump straight into a minigame by id (used for on-device testing). */
    external fun debugStart(levelId: String)

    /** Fills [buffer] with interleaved stereo 16-bit PCM ([frames] frames). Called from the audio thread. */
    external fun renderAudio(buffer: ShortArray, frames: Int)
}
