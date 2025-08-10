import Dockerode from 'dockerode';
import { Readable } from 'stream';
import * as tar from 'tar';
import fs from 'fs';
import path from 'path';

interface Volume {
    host: string;
    container: string;
}

interface Env {
    [key: string]: string;
}

class PodmanClient {
    private docker: Dockerode;

    constructor(url: string) {
        if (url.startsWith('http://')) {
            const parsed = new URL(url);
            this.docker = new Dockerode({ host: parsed.hostname, port: parsed.port || '80' });
        } else {
            this.docker = new Dockerode({ socketPath: url });
        }
    }

    async build(tag: string, context: string, dockerfilePath: string): Promise<void> {
        const files = fs.readdirSync(context);
        // @ts-ignore
        await this.docker.buildImage({ context, src: files }, { t: tag, dockerfile: dockerfilePath });
    }

    async buildTar(tag: string, tarBinaryData: Buffer, dockerfilePath: string): Promise<void> {
        const stream = Readable.from(tarBinaryData);
        // @ts-ignore
        await this.docker.buildImage(stream, { t: tag, dockerfile: dockerfilePath });
    }

    async create(
        image: string,
        ports: string[],
        commands: string[],
        env: Env,
        volumes: Volume[],
        networks: string[]
    ): Promise<string> {
        const Env = Object.entries(env).map(([key, value]) => `${key}=${value}`);
        const HostConfig = {
            Binds: volumes.map(({ host, container }) => `${host}:${container}`),
        };
        const NetworkingConfig = {
            EndpointsConfig: networks.reduce((acc, net) => ({ ...acc, [net]: {} }), {} as Record<string, {}>),
        };
        const container = await this.docker.createContainer({
            Image: image,
            Cmd: commands,
            Env,
            HostConfig,
            NetworkingConfig,
        });
        return container.id;
    }

    async start(containerId: string, initStdin?: string): Promise<void> {
        const container = this.docker.getContainer(containerId);
        await container.start();
        if (initStdin) {
            const stream = await container.attach({ stream: true, stdin: true });
            stream.write(initStdin);
        }
    }

    async restart(containerId: string): Promise<void> {
        const container = this.docker.getContainer(containerId);
        await container.restart();
    }

    async stop(containerId: string): Promise<void> {
        const container = this.docker.getContainer(containerId);
        await container.stop();
    }

    async write(containerId: string, chunk: string): Promise<void> {
        const container = this.docker.getContainer(containerId);
        const stream = await container.attach({ stream: true, stdin: true });
        stream.write(chunk);
    }

    async getName(containerId: string): Promise<string> {
        const container = this.docker.getContainer(containerId);
        const info = await container.inspect();
        return info.Name;
    }

    async createNetwork(name: string): Promise<void> {
        await this.docker.createNetwork({ Name: name });
    }

    async attachOutputs(
        containerId: string,
        stdoutCb: (chunk: string) => void,
        stderrCb: (chunk: string) => void
    ): Promise<NodeJS.WritableStream> {
        const container = this.docker.getContainer(containerId);
        const stream = await container.attach({ stream: true, stdout: true, stderr: true });
        container.modem.demuxStream(stream, {
            write(chunk: Buffer) {
                stdoutCb(chunk.toString('utf-8'));
            },
        }, {
            write(chunk: Buffer) {
                stderrCb(chunk.toString('utf-8'));
            },
        });
        return stream;
    }

    async run(
        image: string,
        ports: string[],
        commands: string[],
        env: Env,
        volumes: Volume[],
        networks: string[],
        initStdin: string
    ): Promise<string> {
        const id = await this.create(image, ports, commands, env, volumes, networks);
        await this.start(id, initStdin);
        return id;
    }
}

export default PodmanClient;