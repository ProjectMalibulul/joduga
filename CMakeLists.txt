cmake_minimum_required(VERSION 3.20)
project(joduga_audio_engine LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_VISIBILITY_PRESET default)

# Detect OS and set platform-specific flags
if(UNIX AND NOT APPLE)
  # Linux: aggressive optimization, SIMD, fast math
  set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -march=native -ffast-math -ftree-vectorize")
  set(RT_LIB pthread rt)
  set(PLATFORM_SOURCES cpp/src/platform/linux_rt.cpp)
elseif(WIN32)
  # Windows: AVX2, fast math, visibility
  set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /arch:AVX2 /fp:fast")
  set(RT_LIB)
  set(PLATFORM_SOURCES cpp/src/platform/windows_rt.cpp)
endif()

# Core audio engine library (shared object exposed to Rust)
add_library(joduga_audio SHARED
  cpp/src/audio_engine.cpp
  cpp/src/audio_node.cpp
  cpp/src/nodes/oscillator.cpp
  cpp/src/nodes/filter.cpp
  cpp/src/nodes/gain.cpp
  ${PLATFORM_SOURCES}
)

target_include_directories(joduga_audio
  PUBLIC cpp/include
)

target_link_libraries(joduga_audio
  PRIVATE ${RT_LIB}
)

# Ensure C ABI is used for FFI
set_target_properties(joduga_audio PROPERTIES
  LINKER_LANGUAGE CXX
  VERSION 0.1.0
  SOVERSION 0
)

# Install target (required by Rust cmake crate)
install(TARGETS joduga_audio
  LIBRARY DESTINATION lib
  ARCHIVE DESTINATION lib
  RUNTIME DESTINATION bin)
