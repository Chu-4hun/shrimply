cmake_minimum_required(VERSION 3.24)
project(unused-gtest LANGUAGES CXX)

add_library(gtest STATIC dummy.cpp)
add_library(gtest_main STATIC dummy.cpp)
add_library(gmock STATIC dummy.cpp)
add_library(gmock_main STATIC dummy.cpp)
