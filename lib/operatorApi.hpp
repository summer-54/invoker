#pragma once
#include "lib/socket.hpp"
#include <functional>
#include <vector>
#include <string>
#include <map>
#include <memory>
#include <sstream>

class OperatorApi: public std::enable_shared_from_this<OperatorApi> {
public:
    enum class STDOUT {none, onEnd, normal};

    enum class Verdict {OK, WA, TL, ML, ITL, RTL, RML, CE, ERR};

protected:
    Socket::Connection* connection;
    int imagesCount = 0, containersCount = 0;
    std::vector<std::function<void(const std::string&)>> callbacks;
    const std::string separatorStr = "----++++====message end====++++----";

    explicit OperatorApi(Socket::Connection* connection);

    void init();

    static std::string stringValue(STDOUT value);
    static std::string stringValue(Verdict value);

    class Container;

    class ContainerTemplate {
    protected:
        int image;
        const std::shared_ptr<OperatorApi> operatorApi;

    public:
        STDOUT stdout = STDOUT::normal, stderr = STDOUT::onEnd;
        std::vector<std::pair<std::string, std::string>> volumes;
        std::map<std::string, std::string> env;
        std::vector<std::string> networks;
        std::string initStdin;

        explicit ContainerTemplate(int image, std::shared_ptr<OperatorApi> operatorApi);

        void onStdout(const std::function<void(const std::string&)>& callback) const;
        void onStderr(const std::function<void(const std::string&)>& callback) const;

        template<typename Type>
        void operator<<(const Type& chunk) {
            std::ostringstream stream;
            stream << chunk;
            initStdin += stream.str();
        }

        Container* run();
    };

    class Container {
    protected:
        int id;
        ContainerTemplate* containerTemplate;
        const std::shared_ptr<OperatorApi> operatorApi;

    public:
        explicit Container(int id, ContainerTemplate* containerTemplate, std::shared_ptr<OperatorApi> operatorApi);

        void onStdout(const std::function<void(const std::string&)>& callback) const;
        void onStderr(const std::function<void(const std::string&)>& callback) const;

        void restart() const;

        void stop() const;

        void write(const std::string& chunk) const;

        template<typename Type>
        void operator<<(const Type& chunk) {
            std::ostringstream stream;
            stream << chunk;
            write(stream.str());
        }

        void getHost(const std::function<void(const std::string&)>& callback) const;
    };

public:
    static void create(const std::string& path, const std::string& initToken, std::function<void(std::shared_ptr<OperatorApi>)> callback);

    std::function<ContainerTemplate*(std::shared_ptr<OperatorApi>)> build(const std::string& context, const std::string& dockerfilePath); // Modified signature

    void setVerdict(const std::string& subtaskId, Verdict verdict, const std::string& data = "") const;
    void setVerdict(Verdict verdict, const std::string& data = "") const;
};