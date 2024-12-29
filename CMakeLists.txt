cmake_minimum_required(VERSION 3.10)

# Project name
project(MyProject LANGUAGES CXX)

# Set default build type to Debug
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Debug)
endif()

set(PLATFORM "x64")

# Set platform-specific flags
# if(CMAKE_SYSTEM_PROCESSOR MATCHES "x86_64")
#     add_compile_options(-m64 -lrt)
# elif(CMAKE_SYSTEM_PROCESSOR MATCHES "i686")
#     set(PLATFORM "x86")
#     add_compile_options(-m32 -lrt)
# elif(CMAKE_SYSTEM_PROCESSOR MATCHES "armv5")
#     set(PLATFORM "armv5")
#     set(CMAKE_CXX_COMPILER "arm-none-linux-gnueabi-g++")
#     add_compile_options(-march=armv5 -lrt)
# elif(CMAKE_SYSTEM_PROCESSOR MATCHES "armv6")
#     set(PLATFORM "armv6")
#     set(CMAKE_CXX_COMPILER "arm-bcm2708hardfp-linux-gnueabi-g++")
#     add_compile_options(-march=armv6 -lrt)
# elif(CMAKE_SYSTEM_PROCESSOR MATCHES "armv7")
#     set(PLATFORM "armv7")
#     set(CMAKE_CXX_COMPILER "arm-linux-gnueabihf-g++")
#     add_compile_options(-march=armv7 -mcpu=cortex-m3 -mthumb)
# elif(CMAKE_SYSTEM_PROCESSOR MATCHES "aarch64")
#     set(PLATFORM "armv8")
#     set(CMAKE_CXX_COMPILER "aarch64-linux-gnu-g++")
#     add_compile_definitions(_LIN _ARM)
# endif()

# Define common compile flags
add_compile_definitions(_LIN)
# if(CMAKE_BUILD_TYPE STREQUAL "Debug")
#     add_compile_definitions(_DEBUG)
#     add_compile_options(-g)
# else()
#     add_compile_options(-O3)
# endif()

# Include and link paths
include_directories(./include)
link_directories(./lib/${PLATFORM})

# OpenCV configuration
find_package(OpenCV REQUIRED)
include_directories(${OpenCV_INCLUDE_DIRS})

# Define source files for targets
set(SOURCES_SNAP demo/main_SDK2_snap.cpp)
set(SOURCES_VIDEO demo/main_SDK2_video.cpp)
set(SOURCES_VIDEO_MAC demo/main_SDK2_video_mac.cpp)

# Add executables
add_executable(test_gui2_snap ${SOURCES_SNAP})
target_link_libraries(test_gui2_snap ${OpenCV_LIBS} ASICamera2 pthread)

add_executable(test_gui2_video ${SOURCES_VIDEO})
target_link_libraries(test_gui2_video ${OpenCV_LIBS} ASICamera2 pthread)

add_executable(main_SDK2_video_mac ${SOURCES_VIDEO_MAC})
target_link_libraries(main_SDK2_video_mac ${OpenCV_LIBS} ASICamera2 pthread)

add_executable(main_websocket demo/main_websocket.cpp)
target_link_libraries(main_websocket ${OpenCV_LIBS} ASICamera2 pthread)
target_include_directories(main_websocket PUBLIC websocketpp)


# Output executables to bin/<platform>/
# set_target_properties(test_gui2_snap PROPERTIES RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin/${PLATFORM})
# set_target_properties(test_gui2_video PROPERTIES RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin/${PLATFORM})
# set_target_properties(main_SDK2_video_mac PROPERTIES RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin/${PLATFORM})

# Clean target
# add_custom_target(clean-all COMMAND ${CMAKE_COMMAND} --build . --target clean)
