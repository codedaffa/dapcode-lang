cmake_minimum_required(VERSION 3.22.1)
project("dapcode_kalkulator")

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -O3 -Wall")

add_library(dapcode_app SHARED
    dapcode_engine.c
    app_dapcode.c
)

target_compile_definitions(dapcode_app PRIVATE
    DAPCODE_CUSTOM_CANVAS=1
    DAPCODE_NO_MAIN=1
)

target_include_directories(dapcode_app PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
)

find_library(log-lib log)
find_library(android-lib android)
find_library(gles2-lib GLESv2)
find_library(egl-lib EGL)
find_library(m-lib m)

target_link_libraries(dapcode_app
    ${log-lib}
    ${android-lib}
    ${gles2-lib}
    ${egl-lib}
    ${m-lib}
)
