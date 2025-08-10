import path from 'path';
import { Connection } from './socket';
import Task from './task';
import {podmanClient, client} from "./main";

let sessionsCount = 0;

function getImageTag(session: number, id: number): string {
    return Date.now() + '-' + session + '-' + id;
}

function getContainerName(session: number, id: number, image: number): string {
    return Date.now() + '_' + session + '_' + image + '_' + id;
}

interface Volume {
    host: string;
    container: string;
}

interface Env {
    [key: string]: string;
}

class Session {
    private id: number;
    private images: Map<number, string>;
    private containers: Map<number, string>;
    private revImages: Map<string, number>;
    private revContainers: Map<string, number>;
    private connection: Connection;
    private volumePath: string;
    networks: Record<string, string>;
    private task: Task;

    constructor(
        networks: Record<string, string>,
        volumePath: string,
        connection: Connection,
        task: Task,
        id: number = sessionsCount++
    ) {
        this.id = id;
        this.images = new Map();
        this.containers = new Map();
        this.revImages = new Map();
        this.revContainers = new Map();
        this.connection = connection;
        this.volumePath = volumePath;
        this.networks = networks;
        this.task = task;
    }

    async onData(data: string): Promise<void> {
        if (!data) return;
        console.log('Received: ' + data);
        const lines: Array<string> = data.split('\n');
        if (lines.length === 0) return;
        const firstLine: Array<string> = (lines[0] || '').split(' ');
        if (firstLine.length < 1) return;
        const type = firstLine[0];
        let lineIdx = 1;

        if (type === 'BUILD') {
            if (firstLine.length < 2 || firstLine[1] === undefined) return;
            const imageId = parseInt(firstLine[1]);
            if (isNaN(imageId)) return;
            const context = lines[lineIdx++] || '';
            const dockerfile = lines[lineIdx++] || '';
            await this.build(imageId, context, dockerfile);
        } else if (type === 'RUN') {
            if (firstLine.length < 3 || firstLine[1] === undefined || firstLine[2] === undefined) return;
            const id = parseInt(firstLine[1]);
            if (isNaN(id)) return;
            const imageId = parseInt(firstLine[2]);
            if (isNaN(imageId)) return;
            let stdout: string = 'normal';
            let stderr: string = 'onEnd';
            const volumes: Volume[] = [];
            const env: Env = {};
            const networks: string[] = [];
            let initStdin: string = '';
            let subIdx = 3;

            while (subIdx < firstLine.length) {
                const subtype = firstLine[subIdx++];
                if (subtype === 'STDOUT') {
                    if (subIdx >= firstLine.length || firstLine[subIdx] === undefined) return;
                    stdout = firstLine[subIdx++] ?? "";
                } else if (subtype === 'STDERR') {
                    if (subIdx >= firstLine.length || firstLine[subIdx] === undefined) return;
                    stderr = firstLine[subIdx++] ?? "";
                } else if (subtype === 'VOLUME') {
                    if (lineIdx >= lines.length) return;
                    const from = (lines[lineIdx++] || '').slice(1);
                    if (lineIdx >= lines.length) return;
                    const to = (lines[lineIdx++] || '').slice(1);
                    volumes.push({ host: from, container: to });
                } else if (subtype === 'ENV') {
                    if (subIdx >= firstLine.length || firstLine[subIdx] === undefined) return;
                    const key = firstLine[subIdx++];
                    if (lineIdx >= lines.length) return;
                    const value = (lines[lineIdx++] || '').slice(1);
                    if (key !== undefined) env[key] = value;
                } else if (subtype === 'NETWORK') {
                    if (lineIdx >= lines.length) return;
                    const network = (lines[lineIdx++] || '').slice(1);
                    networks.push(network);
                } else if (subtype === 'WRITE') {
                    initStdin = lines.slice(lineIdx).join('\n');
                    break;
                } else {
                    console.log('Unknown subtype: ' + subtype);
                }
            }
            await this.run(id, imageId, stdout, stderr, networks, volumes, env, initStdin);
        } else if (type === 'RESTART') {
            if (firstLine.length < 2 || firstLine[1] === undefined) return;
            const id = parseInt(firstLine[1]);
            if (isNaN(id)) return;
            await this.restart(id);
        } else if (type === 'STOP') {
            if (firstLine.length < 2 || firstLine[1] === undefined) return;
            const id = parseInt(firstLine[1]);
            if (isNaN(id)) return;
            await this.stop(id);
        } else if (type === 'WRITE') {
            if (firstLine.length < 2 || firstLine[1] === undefined) return;
            const id = parseInt(firstLine[1]);
            if (isNaN(id)) return;
            const buffer = lines.slice(1).join('\n');
            await this.write(id, buffer);
        } else if (type === 'HOST') {
            if (firstLine.length < 2 || firstLine[1] === undefined) return;
            const id = parseInt(firstLine[1]);
            if (isNaN(id)) return;
            await this.getHost(id);
        } else if (type === 'VERDICT') {
            if (firstLine.length < 2 || firstLine[1] === undefined) return;
            const verdict = firstLine[1];
            let subtask: string | null = null;
            let subIdx = 2;
            if (subIdx < firstLine.length && firstLine[subIdx] === 'SUB') {
                subIdx += 1;
                if (subIdx >= firstLine.length || firstLine[subIdx] === undefined) return;
                subtask = firstLine[subIdx] ?? "";
                subIdx += 1;
            }
            let data_ = '';
            if (subIdx < firstLine.length && firstLine[subIdx] === 'DATA') {
                data_ = lines.slice(1).join('\n');
            }
            this.verdict(verdict, subtask, data_);
        } else {
            console.log('Unknown type: ' + type);
        }
    }

    async build(image: number, context: string, dockerfilePath: string): Promise<void> {
        const tag = getImageTag(this.id, image);
        this.images.set(image, tag);
        this.revImages.set(tag, image);
        await podmanClient.build(tag, context, dockerfilePath);
    }

    async run(
        id: number,
        image: number,
        stdoutMode: string,
        stderrMode: string,
        networks: string[],
        volumes: Volume[],
        env: Env,
        initStdin: string
    ): Promise<void> {
        const mappedNetworks = networks.map((net) => this.networks[net] || net);
        const mappedVolumes = volumes.map(({ host, container }) => ({
            host: path.join(this.volumePath, host),
            container,
        }));
        const containerId = await podmanClient.create(this.images.get(image)!, [], [], env, mappedVolumes, mappedNetworks);
        this.containers.set(id, containerId);
        this.revContainers.set(containerId, id);
        await podmanClient.start(containerId, initStdin);

        if (stdoutMode !== 'none' || stderrMode !== 'none') {
            let stdoutCollected = '';
            let stderrCollected = '';
            const stdoutCb = (chunk: string) => {
                if (stdoutMode === 'normal') this.connection.write('STDOUT ' + id + '\n' + chunk);
                else if (stdoutMode === 'onEnd') stdoutCollected += chunk;
            };
            const stderrCb = (chunk: string) => {
                if (stderrMode === 'normal') this.connection.write('STDERR ' + id + '\n' + chunk);
                else if (stderrMode === 'onEnd') stderrCollected += chunk;
            };
            const stream = await podmanClient.attachOutputs(containerId, stdoutCb, stderrCb);
            stream.on('end', () => {
                if (stdoutMode === 'onEnd' && stdoutCollected) this.connection.write('STDOUT ' + id + '\n' + stdoutCollected);
                if (stderrMode === 'onEnd' && stderrCollected) this.connection.write('STDERR ' + id + '\n' + stderrCollected);
            });
        }
    }

    async restart(id: number): Promise<void> {
        await podmanClient.restart(this.containers.get(id)!);
    }

    async stop(id: number): Promise<void> {
        await podmanClient.stop(this.containers.get(id)!);
    }

    async write(id: number, chunk: string): Promise<void> {
        await podmanClient.write(this.containers.get(id)!, chunk);
    }

    async getHost(id: number): Promise<void> {
        const name = await podmanClient.getName(this.containers.get(id)!);
        this.connection.write('HOST ' + name);
    }

    verdict(verdict: string, subtask: string | null, data: string): void {
        if (subtask) {
            client.sendSubtaskVerdict(this.task.getId(), subtask, verdict, data);
        } else {
            client.sendFullVerdict(this.task.getId(), verdict, data);
        }
    }
}

export default Session;