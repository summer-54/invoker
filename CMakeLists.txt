cmake_minimum_required(VERSION 3.31)
project(invoker)

set(CMAKE_CXX_STANDARD 23)

find_package(fmt REQUIRED)
find_package(libuv REQUIRED)
find_package(Boost REQUIRED COMPONENTS filesystem system)
find_package(CURL REQUIRED)
find_package(nlohmann_json CONFIG REQUIRED)
find_package(httplib CONFIG REQUIRED)
find_package(LibArchive REQUIRED)

link_directories(/home/sizoff/.vcpkg-clion/vcpkg/installed/x64-linux/debug/lib)

add_executable(invoker main.cpp lib/lib/socket.cpp lib/lib/socket.hpp asyncProcess.cpp asyncProcess.hpp session.cpp
        session.hpp lib/podmanClient.cpp lib/podmanClient.hpp websocketClient.cpp websocketClient.hpp task.cpp task.hpp
        lib/tar.cpp
        lib/tar.hpp)
target_link_libraries(invoker PRIVATE fmt::fmt uv Boost::filesystem Boost::system CURL::libcurl nlohmann_json::nlohmann_json httplib::httplib LibArchive::LibArchive)
target_compile_options(invoker PRIVATE
        -fsanitize=address,undefined,leak
        -fno-omit-frame-pointer
        -g
)
target_link_options(invoker PRIVATE
        -fsanitize=address,undefined,leak
)

add_executable(operator client.cpp lib/lib/socket.cpp lib/lib/socket.hpp lib/operatorApi.cpp lib/operatorApi.hpp)
target_link_libraries(operator PRIVATE fmt::fmt uv)
#target_compile_options(operator PRIVATE
#        -fsanitize=address,undefined,leak
#        -fno-omit-frame-pointer
#        -g
#)
#target_link_options(operator PRIVATE
#        -fsanitize=address,undefined,leak
#)