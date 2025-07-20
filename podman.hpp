#pragma once

#include <vector>
#include <string>
#include <map>

void build(const std::string& image, const std::string& context, const std::string& dockerfilePath);

void run(const std::string& id, const std::string& image, const std::string& stdout, const std::string& stderr, const std::vector<int>& ports, const std::vector<std::pair<std::string, std::string>>& volumes, const std::map<std::string, std::string>& env, const std::string& initStdin);

void restart(const std::string& id);

void stop(const std::string& id);

void write(const std::string& id, const std::string& chunk);