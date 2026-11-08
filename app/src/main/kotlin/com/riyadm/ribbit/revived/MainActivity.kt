package com.riyadm.ribbit.revived

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.HapticFeedbackConstants
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.window.OnBackInvokedDispatcher

class MainActivity : Activity() {

    private lateinit var gameView: GameView
    private val audio = AudioEngine()
    private lateinit var updates: UpdateChecker

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        if (Build.VERSION.SDK_INT >= 30) window.setDecorFitsSystemWindows(false)
        window.attributes = window.attributes.apply {
            layoutInDisplayCutoutMode = if (Build.VERSION.SDK_INT >= 30) {
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
            } else {
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
        }

        val font = FontAtlas.build()
        NativeBridge.init(filesDir.absolutePath, font.pixels, font.width, font.height, font.glyphs, font.fontPx, audio.sampleRate)

        gameView = GameView(this, ::handleRequests)
        updates = UpdateChecker(this) { v -> gameView.queueEvent { NativeBridge.setUpdate(v) } }
        NativeBridge.setVersion(updates.current)
        setContentView(gameView)
        gameView.setOnApplyWindowInsetsListener { v, insets -> applyInsets(v, insets) }

        if (Build.VERSION.SDK_INT >= 33) {
            onBackInvokedDispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT) {
                gameView.queueEvent { NativeBridge.back() }
            }
        }
        hideSystemBars()
        handleDebugIntent(intent)
    }

    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        handleDebugIntent(intent)
    }

    private fun handleDebugIntent(intent: android.content.Intent?) {
        val debuggable = applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE != 0
        val level = intent?.getStringExtra("level") ?: return
        if (debuggable) gameView.queueEvent { NativeBridge.debugStart(level) }
    }

    @Deprecated("Used only below API 33; newer versions use OnBackInvokedCallback.")
    override fun onBackPressed() {
        gameView.queueEvent { NativeBridge.back() }
    }

    private fun applyInsets(v: View, insets: WindowInsets): WindowInsets {
        var l = 0
        var t = 0
        var r = 0
        var b = 0
        insets.displayCutout?.let {
            l = it.safeInsetLeft
            t = it.safeInsetTop
            r = it.safeInsetRight
            b = it.safeInsetBottom
        }
        gameView.queueEvent { NativeBridge.setInsets(l, t, r, b) }
        return insets
    }

    private fun hideSystemBars() {
        if (Build.VERSION.SDK_INT >= 30) {
            window.insetsController?.let {
                it.hide(WindowInsets.Type.systemBars())
                it.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        } else {
            @Suppress("DEPRECATION")
            window.decorView.systemUiVisibility = (View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                or View.SYSTEM_UI_FLAG_FULLSCREEN
                or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                or View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION)
        }
    }

    private fun handleRequests(req: Int) {
        if (req and NativeBridge.REQ_HAPTIC_TICK != 0) {
            gameView.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
        }
        if (req and NativeBridge.REQ_HAPTIC_CONFIRM != 0) {
            gameView.performHapticFeedback(
                if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.CONFIRM else HapticFeedbackConstants.VIRTUAL_KEY
            )
        }
        if (req and NativeBridge.REQ_HAPTIC_REJECT != 0) {
            gameView.performHapticFeedback(
                if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.REJECT else HapticFeedbackConstants.LONG_PRESS
            )
        }
        if (req and NativeBridge.REQ_HAPTIC_HEAVY != 0) {
            gameView.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
        }
        if (req and NativeBridge.REQ_CHECK_UPDATE != 0) {
            updates.check()
        }
        if (req and NativeBridge.REQ_OPEN_UPDATE != 0) {
            try {
                startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(updates.downloadUrl)))
            } catch (_: ActivityNotFoundException) {
                // no browser installed
            }
        }
        if (req and NativeBridge.REQ_EXIT != 0) {
            finishAndRemoveTask()
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) hideSystemBars()
    }

    override fun onResume() {
        super.onResume()
        gameView.onResume()
        NativeBridge.resume()
        audio.start()
    }

    override fun onPause() {
        NativeBridge.pause() // engine state is mutex-guarded, safe from the UI thread
        gameView.onPause()
        audio.stop()
        NativeBridge.save()
        super.onPause()
    }
}
