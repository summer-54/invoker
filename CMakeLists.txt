cmake_minimum_required(VERSION 3.31)
project(invoker)

set(CMAKE_CXX_STANDARD 23)

#include(FetchContent)
#
#FetchContent_Declare(
#        dockerpp
#        GIT_REPOSITORY https://github.com/zeroc-ice/docker-cpp.git
#        GIT_TAG        main
#        SOURCE_SUBDIR  cpp
#)
#
#FetchContent_MakeAvailable(dockerpp)

find_package(fmt REQUIRED)
find_package(libuv REQUIRED)
find_package(Boost REQUIRED COMPONENTS filesystem system)
find_package(CURL REQUIRED)
find_package(nlohmann_json CONFIG REQUIRED)
find_package(httplib CONFIG REQUIRED)
find_package(LibArchive REQUIRED)

link_directories(/home/sizoff/.vcpkg-clion/vcpkg/installed/x64-linux/debug/lib)

add_executable(invoker main.cpp lib/lib/socket.cpp lib/lib/socket.hpp asyncProcess.cpp asyncProcess.hpp session.cpp session.hpp lib/podmanClient.cpp lib/podmanClient.hpp)
target_link_libraries(invoker PRIVATE fmt::fmt uv Boost::filesystem Boost::system CURL::libcurl nlohmann_json::nlohmann_json httplib::httplib LibArchive::LibArchive)
add_executable(operator client.cpp lib/lib/socket.cpp lib/lib/socket.hpp lib/operatorApi.cpp lib/operatorApi.hpp)
target_link_libraries(operator PRIVATE fmt::fmt uv)