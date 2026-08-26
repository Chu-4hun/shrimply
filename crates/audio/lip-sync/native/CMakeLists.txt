cmake_minimum_required(VERSION 3.24)
project(shrimply-rhubarb LANGUAGES C CXX)

if(NOT RHUBARB_SOURCE OR NOT PATCHED_POCKET_SPHINX_TOOLS)
	message(FATAL_ERROR "Rhubarb source and patched recognizer paths are required")
endif()

set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(FETCHCONTENT_SOURCE_DIR_GOOGLETEST "${CMAKE_CURRENT_SOURCE_DIR}/gtest" CACHE PATH "" FORCE)
add_subdirectory("${RHUBARB_SOURCE}/rhubarb" rhubarb)

get_target_property(recognition_sources rhubarb-recognition SOURCES)
list(FILTER recognition_sources EXCLUDE REGEX "pocketSphinxTools\\.cpp$")
list(APPEND recognition_sources "${PATCHED_POCKET_SPHINX_TOOLS}")
set_property(TARGET rhubarb-recognition PROPERTY SOURCES "${recognition_sources}")

add_library(shrimply-rhubarb SHARED shim.cpp)
target_compile_features(shrimply-rhubarb PRIVATE cxx_std_17)
target_include_directories(shrimply-rhubarb PRIVATE "${RHUBARB_SOURCE}/rhubarb/src")
target_link_libraries(shrimply-rhubarb PRIVATE rhubarb-lib)
set_target_properties(shrimply-rhubarb PROPERTIES
	LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}"
)
