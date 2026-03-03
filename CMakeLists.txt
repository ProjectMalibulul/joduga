cmake_minimum_required(VERSION 3.20)
project(joduga_audio_engine LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_VISIBILITY_PRESET default)

# Detect OS and set platform-specific flags
if(UNIX AND NOT APPLE)
  # Linux: optimization, SIMD, fast math (use -march=x86-64-v2 for broad compat)
  set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -march=x86-64-v2 -ffast-math -ftree-vectorize")
  set(RT_LIB pthread rt)
  set(PLATFORM_SOURCES cpp/src/platform/linux_rt.cpp)
elseif(WIN32)
  # Windows: AVX2, fast math, visibility
  set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /arch:AVX2 /fp:fast")
  set(RT_LIB)
  set(PLATFORM_SOURCES cpp/src/platform/windows_rt.cpp)
endif()

# Core audio engine library (static archive linked into Rust binary)
add_library(joduga_audio STATIC
  cpp/src/audio_engine.cpp
  cpp/src/audio_node.cpp
  cpp/src/nodes/oscillator.cpp
  cpp/src/nodes/filter.cpp
  cpp/src/nodes/gain.cpp
  ${PLATFORM_SOURCES}
)

# PIC required because this static lib is linked into a PIE executable
set_target_properties(joduga_audio PROPERTIES
  LINKER_LANGUAGE CXX
  POSITION_INDEPENDENT_CODE ON
)

target_include_directories(joduga_audio
  PUBLIC cpp/include
)

target_link_libraries(joduga_audio
  PRIVATE ${RT_LIB}
)

# Install target (required by Rust cmake crate)
install(TARGETS joduga_audio
  LIBRARY DESTINATION lib
  ARCHIVE DESTINATION lib
  RUNTIME DESTINATION bin)
