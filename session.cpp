#include "session.hpp"
#include <chrono>
#include "lib/podmanClient.hpp"
#include <iostream>
#include <sstream>
#include "lib/lib/socket.hpp"

#ifdef PODMAN_SOCKET
PodmanClient podmanClient(PODMAN_SOCKET);
#else
PodmanClient podmanClient("http://localhost:8888");
#endif

std::string getImageTag(int session, int id) {
    return std::to_string(std::chrono::system_clock::now().time_since_epoch().count()) + "-" + std::to_string(session) + "-" + std::to_string(id);
}

int Session::sessionsCount = 0;

Session::Session(Socket::Connection* connection, const int id): connection(connection), id(id) {
    connection->onData([this](const char* data, size_t len) {
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
                build(imageId, context, dockerfile);
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
                run(id, imageId, stdout, stderr, ports, volumes, env, initStdin);
            }
            else if (type == "RESTART") {
                int id; stream >> id;
                restart(id);
            }
            else if (type == "STOP") {
                int id; stream >> id;
                stop(id);
            }
            else if (type == "WRITE") {
                int id; stream >> id;
                std::string buffer, tmp;
                while (std::getline(stream, tmp)) buffer += tmp;
                write(id, buffer);
            }
            else {
                std::cout << "Unknown type: " << type << std::endl;
            }
        });
        connection->onClose([] {
            std::cout << "Connection closed" << '\n' << std::endl;
        });
}

void Session::build(int image, const std::string& context, const std::string& dockerfilePath) {
    std::cerr << "build\n";
    auto tag = getImageTag(id, image);
    images[image] = tag;
    revImages[tag] = image;
    podmanClient.build(tag, context, dockerfilePath);
}

void Session::run(int id, int image, const std::string& stdout, const std::string& stderr, const std::vector<int>& ports, const std::vector<std::pair<std::string, std::string>>& volumes, const std::map<std::string, std::string>& env, const std::string& initStdin) {
    auto tag = podmanClient.create(images[image], {}, {}, env, volumes, initStdin);
    containers[id] = tag;
    revContainers[tag] = id;
    podmanClient.run(tag, {});
}

void Session::restart(int id) {

}

void Session::stop(int id) {

}

void Session::write(int id, const std::string& chunk) {

}

void Session::port(int id, int port) {

}

void Session::verdict(int id, const std::string& sub, const std::string& data) {

}