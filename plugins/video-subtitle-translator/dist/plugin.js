// video-subtitle-translator backend plugin entry
// This module runs in the embedded QuickJS runtime or Deno.
// It provides the `translateVideo` function callable via pluginCall transport.

/**
 * Translate subtitles for a video file.
 *
 * @param {object} params
 * @param {string} params.videoPath - Path to the video file
 * @param {string} params.targetLanguage - Target language code (e.g. "zh", "en")
 * @param {string} [params.sourceLanguage] - Source language (auto-detect if omitted)
 * @param {string} [params.modelId] - Model to use for translation
 * @returns {object} Result with subtitle file path
 */
function translateVideo(params) {
    var videoPath = params.videoPath;
    var targetLanguage = params.targetLanguage;
    var sourceLanguage = params.sourceLanguage || "auto";

    if (!videoPath) {
        throw new Error("videoPath is required");
    }
    if (!targetLanguage) {
        throw new Error("targetLanguage is required");
    }

    // Step 1: Transcribe audio using slab API
    var transcribeResult = Slab.api.request({
        method: "POST",
        path: "/v1/audio/transcriptions",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            file: videoPath,
            language: sourceLanguage !== "auto" ? sourceLanguage : undefined,
        }),
    });

    if (transcribeResult.status !== 200) {
        throw new Error("Transcription failed: " + transcribeResult.body);
    }

    var transcription = JSON.parse(transcribeResult.body);

    // Step 2: Translate segments using chat completion
    var segments = transcription.segments || [];
    var translatedSegments = [];

    for (var i = 0; i < segments.length; i++) {
        var segment = segments[i];
        var translateResult = Slab.api.request({
            method: "POST",
            path: "/v1/chat/completions",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                messages: [
                    {
                        role: "system",
                        content: "You are a subtitle translator. Translate the following subtitle text to " +
                            targetLanguage +
                            ". Output only the translation, nothing else.",
                    },
                    { role: "user", content: segment.text },
                ],
                temperature: 0.3,
            }),
        });

        if (translateResult.status === 200) {
            var chatResp = JSON.parse(translateResult.body);
            var translated =
                chatResp.choices &&
                chatResp.choices[0] &&
                chatResp.choices[0].message &&
                chatResp.choices[0].message.content;
            translatedSegments.push({
                start: segment.start,
                end: segment.end,
                text: translated || segment.text,
            });
        } else {
            // Fallback to original text on failure
            translatedSegments.push(segment);
        }
    }

    // Step 3: Render subtitle file
    var renderResult = Slab.api.request({
        method: "POST",
        path: "/v1/subtitle/render",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            segments: translatedSegments,
            format: "srt",
            videoPath: videoPath,
        }),
    });

    if (renderResult.status !== 200) {
        throw new Error("Subtitle render failed: " + renderResult.body);
    }

    var renderResp = JSON.parse(renderResult.body);

    // Emit progress event
    Slab.ui.emit("translateVideo.complete", {
        videoPath: videoPath,
        outputPath: renderResp.outputPath,
        segmentCount: translatedSegments.length,
    });

    return {
        outputPath: renderResp.outputPath,
        segmentCount: translatedSegments.length,
        targetLanguage: targetLanguage,
    };
}

module.exports = { translateVideo: translateVideo };
