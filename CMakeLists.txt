cmake_minimum_required(VERSION 3.31)
project(cppTestInvoker)

set(CMAKE_CXX_STANDARD 23)

find_package(fmt REQUIRED)
find_package(libuv REQUIRED)
find_package(Boost REQUIRED COMPONENTS filesystem system)

add_executable(cppTestInvoker main.cpp socket.cpp socket.hpp asyncProcess.cpp asyncProcess.hpp)
target_link_libraries(cppTestInvoker PRIVATE fmt::fmt uv Boost::filesystem Boost::system)
add_executable(client client.cpp socket.cpp socket.hpp)
target_link_libraries(client PRIVATE fmt::fmt uv)