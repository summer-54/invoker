import fs from 'fs';
import path from 'path';
import Tar from './tar';
import {podmanClient} from "./main";
import type Session from "./session.ts";

const SOCKET_PATH = '/tmp/invoker.sock';
const SOCKET_INNER_PATH = '/invoker.sock';
const VOLUMES_ROOT = path.join(process.env.HOME || '/root', '.invokerVolumes');

function randomstring(length: number): string {
    const charset = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
    let result = '';
    for (let i = 0; i < length; i++) {
        result += charset[Math.floor(Math.random() * charset.length)];
    }
    return Date.now() + result;
}

function taskImageTag(id: string): string {
    return 'task-' + id + '-' + Date.now();
}

function taskNetworkName(id: string, network: string): string {
    return 'task-' + id + '-' + network + '-' + Date.now() + '-' + randomstring(16);
}

class Task {
    private id: string;
    private initToken: string;
    private operatorContainer: string | null;
    private volumePath: string | null;
    private networks: Record<string, string>;
    session: Session | null;

    constructor(id: string) {
        this.id = id;
        this.initToken = randomstring(256);
        this.operatorContainer = null;
        this.volumePath = null;
        this.networks = {};
        this.session = null;
    }

    getId() {
        return this.id;
    }

    async init(tarBinaryData: Buffer): Promise<void> {
        const imageTag = taskImageTag(this.id);
        await podmanClient.buildTar(imageTag, tarBinaryData, './Dockerfile');
        const tarObj = await Tar.create(tarBinaryData);
        let networksList: string[] = [];
        const [exists, isDir] = tarObj.contains('networks');
        if (exists && !isDir) {
            networksList = tarObj.extract('networks').trim().split(/\s+/).filter(Boolean);
        }
        for (const network of networksList) {
            const netName = taskNetworkName(this.id, network);
            this.networks[network] = netName;
            await podmanClient.createNetwork(netName);
        }
        this.volumePath = path.join(VOLUMES_ROOT, imageTag);
        if (!fs.existsSync(VOLUMES_ROOT)) fs.mkdirSync(VOLUMES_ROOT);
        fs.mkdirSync(this.volumePath, { recursive: true });
        this.operatorContainer = await podmanClient.run(
            imageTag,
            [],
            [],
            { INIT_TOKEN: this.initToken, SOCKET_PATH: SOCKET_INNER_PATH },
            [{host: SOCKET_PATH, container: SOCKET_INNER_PATH}, {host: this.volumePath, container: '/volume'}],
            Object.values(this.networks),
            ''
        );
    }

    stop(): void {
        // Cleanup can be added here
    }

    getToken(): string {
        return this.initToken;
    }

    getNetworks(): Record<string, string> {
        return this.networks;
    }

    getVolumePath(): string {
        return this.volumePath!;
    }

    static async create(id: string, tarBinaryData: Buffer): Promise<Task> {
        const t = new Task(id);
        await t.init(tarBinaryData);
        return t;
    }
}

export default Task;