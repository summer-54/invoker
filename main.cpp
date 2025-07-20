// int main() {
//     try {
//         // AsyncProcess process("python -i");
//         //
//         // process.onStdout([](const std::string& data) {
//         //     std::cout << data;
//         // });
//         //
//         // process.onStderr([](const std::string& data) {
//         //     std::cerr << data;
//         // });
//         //
//         // process.onEnd([](int exit_code) {
//         //     std::cout << "\nProcess ended with code: " << exit_code << std::endl;
//         // });
//         //
//         // // Give Python time to start
//         // // std::this_thread::sleep_for(std::chrono::milliseconds(500));
//         //
//         // // Use the public in stream for synchronous writes
//         // process.in << "print('Hello from Python!')" << std::endl;
//         // process.in << "x = 42" << std::endl;
//         // process.in << "print(f'x squared is {x*x}')" << std::endl;
//         // process.in << "exit()" << std::endl;
//         //
//         // // Keep main thread alive while process runs
//         // while (process.running()) {
//         //     std::this_thread::sleep_for(std::chrono::milliseconds(100));
//         // }
//         AsyncProcess process("tree", "../../..");
//         process.onStdout([](auto data) {
//             std::cout << data;
//         });
//         while (process.running()) {
//             std::this_thread::sleep_for(std::chrono::milliseconds(100));
//         }
//
//     } catch (const std::exception& e) {
//         std::cerr << "Error: " << e.what() << std::endl;
//         return 1;
//     }
//
//     return 0;
// }

#include <chrono>
#include <iostream>
#include <sstream>

#include "lib/lib/socket.hpp"
#include "podman.hpp"

std::map<int, std::map<int, std::string>> images, containers;

std::string getContainerId(int session, int id) {
    if (!containers[session].contains(id))
        containers[session][id] = std::to_string(std::chrono::system_clock::now().time_since_epoch().count()) + "-" + std::to_string(session) + "-" + std::to_string(id);
    return containers[session][id];
}

std::string getImageId(int session, int id) {
    if (!images[session].contains(id))
        images[session][id] = std::to_string(std::chrono::system_clock::now().time_since_epoch().count()) + "-" + std::to_string(session) + "-" + std::to_string(id);
    return images[session][id];
}

int main() {
    int sessions = 0;
    Socket::Server server("/tmp/mySocket");
    server.onConnect([&sessions](Socket::Connection* conn) {
        int session = sessions++;
        conn->onData([&session](const char* data, size_t len) {
            std::cout << "Received: " << std::string(data, len) << '\n' << std::endl;
            auto chunk = std::string(data, len);
            std::istringstream stream(chunk);
            std::string type; stream >> type;
            if (type == "BUILD") {
                int imageId; stream >> imageId;
                std::string context, dockerfile;
                std::getline(stream, context);
                std::getline(stream, context);
                std::getline(stream, dockerfile);
                build(getImageId(session, imageId), context, dockerfile);
            }
            else if (type == "RUN") {
                int id, imageId; stream >> id >> imageId;
                std::string stdout = "normal", stderr = "onEnd", subtype;
                std::vector<int> ports;
                std::vector<std::pair<std::string, std::string>> volumes;
                std::map<std::string, std::string> env;
                std::string initStdin;
                while (stream >> subtype) {
                    if (subtype == "STDOUT") stream >> stdout;
                    else if (subtype == "STDERR") stream >> stderr;
                    else if (subtype == "PORTS") {
                        std::string portsStr; std::getline(stream, portsStr);
                        std::istringstream stream0(portsStr);
                        int port;
                        while (stream0 >> port) ports.push_back(port);
                    }
                    else if (subtype == "VOLUME") {
                        std::string from, to;
                        std::getline(stream, from);
                        std::getline(stream, to);
                        from.erase(0, 1);
                        volumes.emplace_back(from, to);
                    }
                    else if (subtype == "ENV") {
                        std::string key, value;
                        stream >> key;
                        std::getline(stream, value);
                        value.erase(0, 1);
                        env.emplace(key, value);
                    }
                    else if (subtype == "WRITE") {
                        std::string tmp; std::getline(stream, tmp);
                        while (std::getline(stream, tmp)) initStdin += tmp;
                        break;
                    }
                    else {
                        std::cout << "Error: " << subtype << '\n' << std::endl;
                    }
                }
                run(getContainerId(session, id), getImageId(session, imageId), stdout, stderr, ports, volumes, env, initStdin);
            }
            else if (type == "RESTART") {
                int id; stream >> id;
                restart(getContainerId(session, id));
            }
            else if (type == "STOP") {
                int id; stream >> id;
                stop(getContainerId(session, id));
            }
            else if (type == "WRITE") {
                int id; stream >> id;
                std::string buffer, tmp;
                while (std::getline(stream, tmp)) buffer += tmp;
                write(getContainerId(session, id), buffer);
            }
            else {
                std::cout << "Unknown type: " << type << std::endl;
            }
        });
        conn->onClose([] {
            std::cout << "Connection closed" << '\n' << std::endl;
        });
    });
    server.start();
}