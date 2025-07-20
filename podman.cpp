#include "podman.hpp"

#include <iostream>

void build(const std::string& image, const std::string& context, const std::string& dockerfilePath) {
    std::cout << "build " << image << ' ' << context << ' ' << dockerfilePath << std::endl;
}

void run(const std::string& id, const std::string& image, const std::string& stdout, const std::string& stderr,
    const std::vector<int>& ports, const std::vector<std::pair<std::string, std::string>>& volumes,
    const std::map<std::string, std::string>& env, const std::string& initStdin) {
    std::cout << "run " << id << ' ' << image << ' ' << stdout << ' ' << stderr << '\n';
    for (int port : ports) std::cout << port << ' ';
    std::cout << '\n';
    for (const auto& [a, b] : volumes) std::cout << a << ':' << b << ' ';
    std::cout << '\n';
    for (const auto& [a, b] : env) std::cout << a << ':' << b << ' ';
    std::cout << '\n' << initStdin << '\n' << std::endl;
}

void restart(const std::string& id) {
    std::cout << "restart " << id << std::endl;
}

void stop(const std::string& id) {
    std::cout << "stop " << id << std::endl;
}

void write(const std::string& id, const std::string& chunk) {
    std::cout << "write " << id << ' ' << chunk << std::endl;
}