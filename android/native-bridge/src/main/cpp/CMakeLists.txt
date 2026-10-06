cmake_minimum_required(VERSION 3.22.1)

project("charmorph-native")

# Add Eigen (header only)
# add_library(eigen INTERFACE)
# target_include_directories(eigen INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/third_party/eigen)

add_library(
    charmorph-native
    SHARED
    charmorph_native.cpp
)

find_library(
    log-lib
    log
)

target_link_libraries(
    charmorph-native
    ${log-lib}
)
