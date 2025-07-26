#include "tar.hpp"
#include <archive.h>
#include <archive_entry.h>
#include <stdexcept>
#include <sstream>
#include <iostream>

void Tar::loadArchive() {
    struct archive* a = archive_read_new();
    archive_read_support_filter_all(a);
    archive_read_support_format_all(a);
    
    int r = archive_read_open_memory(a, archiveData.data(), archiveData.size());
    if (r != ARCHIVE_OK) {
        archive_read_free(a);
        throw std::runtime_error("Failed to open archive");
    }
    
    struct archive_entry* entry;
    while (archive_read_next_header(a, &entry) == ARCHIVE_OK) {
        const char* path = archive_entry_pathname(entry);
        if (!path) continue;
        
        std::string entryPath(path);
        
        // Check if it's a directory
        if (archive_entry_filetype(entry) == AE_IFDIR) {
            isDirectory[entryPath] = true;
            fileContents[entryPath] = "";
        } else {
            isDirectory[entryPath] = false;
            
            // Read file content
            std::stringstream content;
            const void* buff;
            size_t size;
            la_int64_t offset;
            
            while (archive_read_data_block(a, &buff, &size, &offset) == ARCHIVE_OK) {
                content.write(static_cast<const char*>(buff), size);
            }
            fileContents[entryPath] = content.str();
        }
    }
    
    archive_read_free(a);
}

void Tar::rebuildArchive() {
    struct archive* a = archive_write_new();
    archive_write_set_format_pax_restricted(a);
    archive_write_add_filter_none(a);
    
    std::stringstream output;
    archive_write_open_memory(a, nullptr, 0, nullptr);
    
    // Write entries
    for (const auto& entry : fileContents) {
        const std::string& path = entry.first;
        const std::string& content = entry.second;
        
        struct archive_entry* ae = archive_entry_new();
        archive_entry_set_pathname(ae, path.c_str());
        
        if (isDirectory[path]) {
            archive_entry_set_filetype(ae, AE_IFDIR);
            archive_entry_set_perm(ae, 0755);
        } else {
            archive_entry_set_filetype(ae, AE_IFREG);
            archive_entry_set_perm(ae, 0644);
            archive_entry_set_size(ae, content.size());
        }
        
        archive_write_header(a, ae);
        
        if (!isDirectory[path] && !content.empty()) {
            archive_write_data(a, content.data(), content.size());
        }
        
        archive_entry_free(ae);
    }
    
    // Get the result
    const void* buff;
    size_t size;
    la_int64_t offset;
    
    std::stringstream result;
    while (archive_write_get_bytes_in_last_block(a) >= 0) {
        if (archive_write_get_bytes_in_last_block(a) == 0) break;
        // This is a simplified approach - in practice you'd need to handle this better
        break;
    }
    
    archive_write_free(a);
    
    // For now, we'll rebuild by creating a new archive
    rebuildArchiveProper();
}

void Tar::rebuildArchiveProper() {
    struct archive* a = archive_write_new();
    archive_write_set_format_pax_restricted(a);
    archive_write_add_filter_none(a);
    
    std::vector<char> buffer;
    buffer.resize(8192);
    
    archive_write_open_memory(a, buffer.data(), buffer.size(), nullptr);
    
    // Write entries
    for (const auto& entry : fileContents) {
        const std::string& path = entry.first;
        const std::string& content = entry.second;
        
        struct archive_entry* ae = archive_entry_new();
        archive_entry_set_pathname(ae, path.c_str());
        
        if (isDirectory[path]) {
            archive_entry_set_filetype(ae, AE_IFDIR);
            archive_entry_set_perm(ae, 0755);
            archive_entry_set_size(ae, 0);
        } else {
            archive_entry_set_filetype(ae, AE_IFREG);
            archive_entry_set_perm(ae, 0644);
            archive_entry_set_size(ae, content.size());
        }
        
        archive_write_header(a, ae);
        
        if (!isDirectory[path] && !content.empty()) {
            archive_write_data(a, content.data(), content.size());
        }
        
        archive_entry_free(ae);
    }
    
    archive_write_close(a);
    
    // Note: This approach has limitations with memory writing
    // In a production implementation, you'd want to use a custom callback
    archive_write_free(a);
    
    // For now, we'll just keep the in-memory representation
}

Tar::Tar(const std::string& binaryData) : archiveData(binaryData) {
    loadArchive();
}

std::vector<std::string> Tar::list(const std::string& path) {
    std::vector<std::string> result;
    std::string prefix = path;
    
    // Ensure path ends with '/' for directory matching
    if (!prefix.empty() && prefix.back() != '/') {
        prefix += '/';
    }
    
    for (const auto& entry : fileContents) {
        const std::string& entryPath = entry.first;
        
        // Skip the directory itself
        if (entryPath == path) continue;
        
        // Check if entry is directly under the specified path
        if (entryPath.substr(0, prefix.length()) == prefix) {
            std::string remainder = entryPath.substr(prefix.length());
            
            // Only include direct children (no nested paths)
            if (remainder.find('/') == std::string::npos || 
                remainder.find('/') == remainder.length() - 1) {
                result.push_back(entryPath);
            }
        }
    }
    
    return result;
}

std::string Tar::extract(const std::string& path) {
    auto it = fileContents.find(path);
    if (it != fileContents.end() && !isDirectory[path]) {
        return it->second;
    }
    throw std::runtime_error("File not found or is a directory: " + path);
}

void Tar::insert(const std::string& path, const std::string& data) {
    fileContents[path] = data;
    isDirectory[path] = false;
    // rebuildArchive(); // Uncomment in production implementation
}

std::pair<bool, bool> Tar::contains(const std::string& path) {
    auto it = fileContents.find(path);
    if (it != fileContents.end()) {
        return std::make_pair(true, isDirectory[path]);
    }
    return std::make_pair(false, false);
}