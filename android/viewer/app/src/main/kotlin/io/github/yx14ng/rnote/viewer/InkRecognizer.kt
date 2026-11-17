package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.net.ConnectivityManager
import com.google.mlkit.common.model.DownloadConditions
import com.google.mlkit.common.model.RemoteModelManager
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognition
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognitionModel
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognitionModelIdentifier
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognizer
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognizerOptions
import com.google.mlkit.vision.digitalink.recognition.Ink
import com.google.mlkit.vision.digitalink.recognition.RecognitionContext
import com.google.mlkit.vision.digitalink.recognition.WritingArea

/** Whether the recognizer of a language can read yet. */
internal sealed interface ModelState {
    data object Ready : ModelState

    data object Downloading : ModelState

    /** Not downloaded, and no connection to download it now. */
    data object Offline : ModelState

    data class Failed(val message: String) : ModelState
}

/** A piece of writing to read: its strokes, each point as (x, y, time in ms). */
internal class InkFragment(
    val hash: String,
    val rect: DoubleArray,
    val strokes: List<List<DoubleArray>>,
)

/**
 * Reads handwriting with ML Kit Digital Ink, in the language of the notebook. Each language
 * has a model of its own, downloaded once. Results come back on the main thread.
 */
internal class InkRecognizer(private val context: Context) : AutoCloseable {
    private val models = RemoteModelManager.getInstance()
    private var current: Pair<InkLanguage, DigitalInkRecognizer>? = null

    /** Makes sure the model of [language] is on the device, downloading it if needed. */
    fun ensureModel(language: InkLanguage, onState: (ModelState) -> Unit) {
        val model = model(language)
        models.isModelDownloaded(model)
            .addOnSuccessListener { downloaded ->
                if (downloaded) {
                    onState(ModelState.Ready)
                    return@addOnSuccessListener
                }
                if (!online()) {
                    onState(ModelState.Offline)
                    return@addOnSuccessListener
                }
                onState(ModelState.Downloading)
                models.download(model, DownloadConditions.Builder().build())
                    .addOnSuccessListener { onState(ModelState.Ready) }
                    .addOnFailureListener { error -> onState(failure(error)) }
            }
            .addOnFailureListener { error -> onState(failure(error)) }
    }

    /**
     * The best readings (up to three) of [fragment] in [language]; [before] is what was
     * read just before it on its line, which helps with words split in two.
     */
    fun recognize(fragment: InkFragment, language: InkLanguage, before: String, done: (Result<List<String>>) -> Unit) {
        val ink = Ink.builder()
        for (points in fragment.strokes) {
            val stroke = Ink.Stroke.builder()
            for (point in points) {
                stroke.addPoint(Ink.Point.create(point[0].toFloat(), point[1].toFloat(), point[2].toLong()))
            }
            ink.addStroke(stroke.build())
        }
        val area = WritingArea(
            (fragment.rect[2] - fragment.rect[0]).toFloat().coerceAtLeast(1f),
            (fragment.rect[3] - fragment.rect[1]).toFloat().coerceAtLeast(1f),
        )
        val recognitionContext = RecognitionContext.builder().setPreContext(before).setWritingArea(area).build()
        recognizer(language).recognize(ink.build(), recognitionContext)
            .addOnSuccessListener { result -> done(Result.success(result.candidates.take(CANDIDATES).map { it.text })) }
            .addOnFailureListener { error -> done(Result.failure(error)) }
    }

    override fun close() {
        current?.second?.close()
        current = null
    }

    private fun recognizer(language: InkLanguage): DigitalInkRecognizer {
        current?.let { (forLanguage, recognizer) -> if (forLanguage == language) return recognizer }
        close()
        val recognizer = DigitalInkRecognition.getClient(DigitalInkRecognizerOptions.builder(model(language)).build())
        current = language to recognizer
        return recognizer
    }

    private fun model(language: InkLanguage): DigitalInkRecognitionModel {
        val identifier = requireNotNull(DigitalInkRecognitionModelIdentifier.fromLanguageTag(language.tag)) {
            "ML Kit has no handwriting model for ${language.tag}"
        }
        return DigitalInkRecognitionModel.builder(identifier).build()
    }

    private fun online(): Boolean =
        context.getSystemService(ConnectivityManager::class.java)?.activeNetwork != null

    private fun failure(error: Exception): ModelState =
        if (online()) ModelState.Failed(error.message ?: error.toString()) else ModelState.Offline

    private companion object {
        const val CANDIDATES = 3
    }
}
