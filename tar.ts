import tarStream from 'tar-stream';
import { Readable } from 'stream';

class Tar {
    private archiveData: Buffer;
    private fileContents: Map<string, string>;
    private isDirectory: Map<string, boolean>;

    constructor(binaryData: Buffer) {
        this.archiveData = binaryData;
        this.fileContents = new Map();
        this.isDirectory = new Map();
    }

    async loadArchive(): Promise<void> {
        return new Promise((resolve, reject) => {
            const extract = tarStream.extract();
            extract.on('entry', (header, stream, next) => {
                const path = header.name;
                if (header.type === 'directory') {
                    this.isDirectory.set(path, true);
                    this.fileContents.set(path, '');
                    next();
                } else {
                    this.isDirectory.set(path, false);
                    const chunks: Buffer[] = [];
                    stream.on('data', (chunk) => chunks.push(chunk));
                    stream.on('end', () => {
                        this.fileContents.set(path, Buffer.concat(chunks).toString('utf-8'));
                        next();
                    });
                }
                stream.resume();
            });
            extract.on('finish', resolve);
            extract.on('error', reject);
            const readable = Readable.from([this.archiveData]);
            readable.pipe(extract);
        });
    }

    list(path: string): string[] {
        let prefix = path;
        if (prefix && prefix[prefix.length - 1] !== '/') prefix += '/';
        const result: string[] = [];
        for (const entryPath of this.fileContents.keys()) {
            if (entryPath === path) continue;
            if (entryPath.startsWith(prefix)) {
                const remainder = entryPath.slice(prefix.length);
                if (!remainder.includes('/') || remainder.endsWith('/')) result.push(entryPath);
            }
        }
        return result;
    }

    extract(path: string): string {
        if (this.fileContents.has(path) && !this.isDirectory.get(path)) return this.fileContents.get(path)!;
        throw new Error('File not found or is a directory: ' + path);
    }

    insert(path: string, data: string): void {
        this.fileContents.set(path, data);
        this.isDirectory.set(path, false);
    }

    contains(path: string): [boolean, boolean] {
        if (this.fileContents.has(path)) return [true, this.isDirectory.get(path)!];
        return [false, false];
    }

    static async create(binaryData: Buffer): Promise<Tar> {
        const t = new Tar(binaryData);
        await t.loadArchive();
        return t;
    }
}

export default Tar;