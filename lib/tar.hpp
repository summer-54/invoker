#ifndef TAR_HPP
#define TAR_HPP

#include <string>
#include <vector>
#include <map>
#include <utility>

class Tar {
private:
    std::string archiveData;
    std::map<std::string, std::string> fileContents;
    std::map<std::string, bool> isDirectory;
    
    // Helper function to load archive data into memory structures
    void loadArchive();
    
    // Helper function to rebuild archive data from memory structures
    void rebuildArchive();
    void rebuildArchiveProper();

public:
    // Constructor
    explicit Tar(const std::string& binaryData);
    
    // List files in a directory path within the archive
    std::vector<std::string> list(const std::string& path);
    
    // Extract file content from the archive
    std::string extract(const std::string& path);
    
    // Insert a new file into the archive
    void insert(const std::string& path, const std::string& data);
    
    // Check if a path exists and whether it's a directory
    std::pair<bool, bool> contains(const std::string& path);
};

#endif // TAR_HPP