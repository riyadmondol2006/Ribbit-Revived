package com.riyadm.ribbit.revived

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack

/**
 * Streams the Rust mixer's output through a low-latency AudioTrack on a dedicated thread.
 * All synthesis (sound effects and music) happens in Rust; this class only moves samples.
 */
class AudioEngine {
    val sampleRate: Int = AudioTrack.getNativeOutputSampleRate(AudioManager.STREAM_MUSIC).takeIf { it in 22050..96000 } ?: 48000

    @Volatile private var running = false
    private var thread: Thread? = null

    fun start() {
        if (running) return
        running = true
        thread = Thread({ loop() }, "ribbit-audio").apply {
            priority = Thread.MAX_PRIORITY
            start()
        }
    }

    fun stop() {
        running = false
        thread?.join(500)
        thread = null
    }

    private fun loop() {
        val minBytes = AudioTrack.getMinBufferSize(sampleRate, AudioFormat.CHANNEL_OUT_STEREO, AudioFormat.ENCODING_PCM_16BIT)
        val framesPerChunk = 256
        val bufferBytes = maxOf(minBytes, framesPerChunk * 4 * 4)
        val track = try {
            AudioTrack.Builder()
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_GAME)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                        .build()
                )
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setSampleRate(sampleRate)
                        .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                        .setChannelMask(AudioFormat.CHANNEL_OUT_STEREO)
                        .build()
                )
                .setBufferSizeInBytes(bufferBytes)
                .setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
                .setTransferMode(AudioTrack.MODE_STREAM)
                .build()
        } catch (e: Exception) {
            running = false
            return
        }
        val chunk = ShortArray(framesPerChunk * 2)
        try {
            track.play()
            while (running) {
                NativeBridge.renderAudio(chunk, framesPerChunk)
                var off = 0
                while (off < chunk.size && running) {
                    val n = track.write(chunk, off, chunk.size - off, AudioTrack.WRITE_BLOCKING)
                    if (n <= 0) break
                    off += n
                }
            }
        } finally {
            try {
                track.pause()
                track.flush()
            } catch (_: IllegalStateException) {
            }
            track.release()
        }
    }
}
