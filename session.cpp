#include "session.hpp"
#include <chrono>
#include <cstring>
#include <httplib.h>

#include "lib/podmanClient.hpp"
#include <iostream>
#include <sstream>
#include "lib/lib/socket.hpp"

PodmanClient podmanClient(std::getenv("PODMAN_SOCKET") == nullptr ? "http://localhost:8888" : std::getenv("PODMAN_SOCKET"));

std::string getImageTag(int session, int id) {
    return std::to_string(std::chrono::system_clock::now().time_since_epoch().count()) + "-" + std::to_string(session) + "-" + std::to_string(id);
}

int findFreePort(int min, int max = 65535) {
    for (int port = min; port <= std::min(max, 65535); port++) {
        httplib::Server svr;

        // Try to bind to the port
        if (svr.bind_to_port("0.0.0.0", port)) {
            // Success - port is available
            svr.stop();
            return port;
        }

        // Port in use, try next one
        svr.stop();
    }

    throw std::runtime_error("No available port found >= " + std::to_string(min));
}

int Session::sessionsCount = 0;

Session::Session(const int id): id(id) {}

void Session::onData(std::string data) {
    std::cout << "Received: " << data << '\n' << std::endl;
    std::istringstream stream(data);
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
    else if (type == "VERDICT") {
        std::string verdict, sub, subtask, data_; stream >> verdict >> sub;
        if (sub == "SUB") {
            std::getline(stream, subtask);
            stream >> sub;
        }
        if (sub == "DATA") {
            std::string tmp; std::getline(stream, tmp);
            while (std::getline(stream, tmp)) data_ += tmp;
        }

    }
    else {
        std::cout << "Unknown type: " << type << std::endl;
    }
}

void Session::build(int image, const std::string& context, const std::string& dockerfilePath) {
    std::cerr << "build\n";
    auto tag = getImageTag(id, image);
    images[image] = tag;
    revImages[tag] = image;
    podmanClient.build(tag, context, dockerfilePath);
}

std::function<void(const std::string&)> stdoutCallback(int id, const std::string& stdout, Socket::Connection* connection) {
    return [&connection, &id](const std::string& chunk) {
        connection->write("STDOUT " + std::to_string(id) + '\n' + chunk);
    };
}

void Session::run(int id, int image, const std::string& stdout, const std::string& stderr, const std::vector<int>& ports, const std::vector<std::pair<std::string, std::string>>& volumes, const std::map<std::string, std::string>& env, const std::string& initStdin) {
    auto containerId = podmanClient.run(images[image], {}, {}, env, volumes, {}, initStdin);
    containers[id] = containerId;
    revContainers[containerId] = id;
    if (stdout != "none" || stderr != "none") podmanClient.attach(containerId);
    // if (stdout != "none") podmanClient.onStdout(containerId, stdoutCallback(id, stdout, connection));
    // if (stderr != "none") podmanClient.onStderr(containerId, stdoutCallback(id, stderr, connection));
}

void Session::restart(int id) {
    podmanClient.restart(containers[id]);
}

void Session::stop(int id) {
    podmanClient.stop(containers[id]);
}

void Session::write(int id, const std::string& chunk) {
    podmanClient.write(containers[id], chunk);
}

void Session::port(int id, int port) {

}

void Session::verdict(int id, const std::string& sub, const std::string& data) {

}