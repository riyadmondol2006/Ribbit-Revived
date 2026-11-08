package com.riyadm.ribbit.revived

import android.annotation.SuppressLint
import android.content.Context
import android.opengl.GLSurfaceView
import android.view.MotionEvent
import javax.microedition.khronos.egl.EGL10
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.egl.EGLDisplay
import javax.microedition.khronos.opengles.GL10

/** OpenGL ES 3 surface whose renderer thread drives the Rust engine once per vsync. */
@SuppressLint("ViewConstructor")
class GameView(context: Context, private val onRequests: (Int) -> Unit) : GLSurfaceView(context) {

    init {
        setEGLContextClientVersion(3)
        setEGLConfigChooser(MsaaConfigChooser())
        preserveEGLContextOnPause = true
        setRenderer(EngineRenderer())
        renderMode = RENDERMODE_CONTINUOUSLY
        isFocusable = true
        isFocusableInTouchMode = true
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                val i = event.actionIndex
                send(NativeBridge.TOUCH_DOWN, event.getPointerId(i), event.getX(i), event.getY(i))
            }
            MotionEvent.ACTION_MOVE -> {
                for (i in 0 until event.pointerCount) {
                    send(NativeBridge.TOUCH_MOVE, event.getPointerId(i), event.getX(i), event.getY(i))
                }
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                val i = event.actionIndex
                send(NativeBridge.TOUCH_UP, event.getPointerId(i), event.getX(i), event.getY(i))
            }
            MotionEvent.ACTION_CANCEL -> {
                for (i in 0 until event.pointerCount) {
                    send(NativeBridge.TOUCH_CANCEL, event.getPointerId(i), event.getX(i), event.getY(i))
                }
            }
        }
        return true
    }

    private fun send(action: Int, id: Int, x: Float, y: Float) {
        queueEvent { NativeBridge.touch(action, id, x, y) }
    }

    private inner class EngineRenderer : Renderer {
        override fun onSurfaceCreated(gl: GL10?, config: EGLConfig?) {
            NativeBridge.surfaceCreated()
        }

        override fun onSurfaceChanged(gl: GL10?, width: Int, height: Int) {
            NativeBridge.surfaceChanged(width, height)
        }

        override fun onDrawFrame(gl: GL10?) {
            val req = NativeBridge.frame(System.nanoTime())
            if (req != 0) post { onRequests(req) }
        }
    }

    /** Prefers 4x MSAA RGBA8888 for smooth edges, falling back to a plain RGB(A) config. */
    private class MsaaConfigChooser : EGLConfigChooser {
        private val esBit = 0x40 // EGL_OPENGL_ES3_BIT_KHR

        override fun chooseConfig(egl: EGL10, display: EGLDisplay): EGLConfig {
            val attempts = listOf(
                intArrayOf(
                    EGL10.EGL_RED_SIZE, 8, EGL10.EGL_GREEN_SIZE, 8, EGL10.EGL_BLUE_SIZE, 8, EGL10.EGL_ALPHA_SIZE, 8,
                    EGL10.EGL_DEPTH_SIZE, 0, EGL10.EGL_STENCIL_SIZE, 0, EGL10.EGL_RENDERABLE_TYPE, esBit,
                    EGL10.EGL_SAMPLE_BUFFERS, 1, EGL10.EGL_SAMPLES, 4, EGL10.EGL_NONE,
                ),
                intArrayOf(
                    EGL10.EGL_RED_SIZE, 8, EGL10.EGL_GREEN_SIZE, 8, EGL10.EGL_BLUE_SIZE, 8,
                    EGL10.EGL_RENDERABLE_TYPE, esBit, EGL10.EGL_NONE,
                ),
                intArrayOf(EGL10.EGL_RENDERABLE_TYPE, esBit, EGL10.EGL_NONE),
            )
            for (attrs in attempts) {
                val num = IntArray(1)
                val configs = arrayOfNulls<EGLConfig>(1)
                if (egl.eglChooseConfig(display, attrs, configs, 1, num) && num[0] > 0 && configs[0] != null) {
                    return configs[0]!!
                }
            }
            throw IllegalStateException("No OpenGL ES 3 config available")
        }
    }
}
