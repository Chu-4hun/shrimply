#include "shim.h"

#include "animation/targetShapeSet.h"
#include "lib/rhubarbLib.h"
#include "recognition/PhoneticRecognizer.h"
#include "recognition/pocketSphinxTools.h"
#include "tools/progress.h"

#include <boost/optional.hpp>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <filesystem>
#include <limits>
#include <mutex>
#include <new>
#include <stdexcept>
#include <string>
#include <vector>

namespace {

std::once_flag model_directory_once;
std::mutex analysis_mutex;
std::filesystem::path configured_model_directory;

uint8_t shape_byte(Shape shape) {
    switch (shape) {
        case Shape::A: return 'A';
        case Shape::B: return 'B';
        case Shape::C: return 'C';
        case Shape::D: return 'D';
        case Shape::E: return 'E';
        case Shape::F: return 'F';
        case Shape::G: return 'G';
        case Shape::H: return 'H';
        case Shape::X: return 'X';
        default: throw std::runtime_error("Rhubarb returned an unknown mouth shape");
    }
}

char* copy_error(const std::string& message) {
    auto* error = static_cast<char*>(std::malloc(message.size() + 1));
    if (!error) return nullptr;
    std::memcpy(error, message.c_str(), message.size() + 1);
    return error;
}

void fail(ShrimplyRhubarbResult* result, const std::string& message) {
    result->error = copy_error(message);
}

} // namespace

const std::filesystem::path& shrimplySphinxModelDirectory() {
    if (configured_model_directory.empty()) {
        throw std::runtime_error("Rhubarb model directory was not configured");
    }
    return configured_model_directory;
}

extern "C" int shrimply_rhubarb_analyze(
    const char* wave_path,
    const char* model_directory,
    int max_thread_count,
    ShrimplyRhubarbResult* result
) {
    if (!result) return 1;
    *result = {};
    try {
        if (!wave_path || !model_directory) {
            throw std::invalid_argument("Rhubarb received a null path");
        }
        if (max_thread_count < 1) {
            throw std::invalid_argument("Rhubarb thread count must be positive");
        }

        const std::lock_guard analysis_lock(analysis_mutex);
        const auto requested_model_directory = std::filesystem::u8path(model_directory);
        std::call_once(model_directory_once, [&] {
            configured_model_directory = requested_model_directory;
        });
        if (configured_model_directory != requested_model_directory) {
            throw std::invalid_argument("Rhubarb model directory changed during this process");
        }
        const ShapeSet shapes {
            Shape::A, Shape::B, Shape::C, Shape::D, Shape::E,
            Shape::F, Shape::G, Shape::H, Shape::X,
        };
        const PhoneticRecognizer recognizer;
        NullProgressSink progress;
        const auto animation = animateWaveFile(
            std::filesystem::u8path(wave_path),
            boost::none,
            recognizer,
            shapes,
            max_thread_count,
            progress
        );

        std::vector<ShrimplyMouthCue> cues;
        cues.reserve(animation.size());
        for (const auto& cue : animation) {
            cues.push_back({
                cue.getStart().count(),
                cue.getEnd().count(),
                shape_byte(cue.getValue()),
            });
        }
        if (!cues.empty()) {
            const size_t bytes = cues.size() * sizeof(ShrimplyMouthCue);
            result->cues = static_cast<ShrimplyMouthCue*>(std::malloc(bytes));
            if (!result->cues) throw std::bad_alloc();
            std::memcpy(result->cues, cues.data(), bytes);
            result->cue_count = cues.size();
        }
        return 0;
    } catch (const std::exception& error) {
        fail(result, error.what());
    } catch (...) {
        fail(result, "Rhubarb failed with an unknown C++ exception");
    }
    return 1;
}

extern "C" void shrimply_rhubarb_free_result(ShrimplyRhubarbResult* result) {
    if (!result) return;
    std::free(result->cues);
    std::free(result->error);
    *result = {};
}
