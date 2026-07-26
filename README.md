# Invoker — Technical Documentation

> Comprehensive reference for sandbox configuration, judging, API protocol, and task structure.

![Icon](icon.png)

---

## Table of Contents

1. [Configuration Files](#configuration-files)
    - [`isolate.yaml`](#isolateyaml)
    - [`judge.yaml`](#judgeyaml)
2. [Environment Variables](#environment-variables)
3. [API Protocol](#api-protocol)
    - [Master Stream](#master-stream)
    - [Auth Stream](#auth-stream)
4. [Verdicts Reference](#verdicts-reference)
5. [Task Structure](#task-structure)
    - [Directory Layout](#directory-layout)
    - [Checker & Interactor](#checker--interactor)
    - [`config.yaml`](#configyaml)
6. [Type Definitions](#type-definitions)

---

## Configuration Files

### `isolate.yaml`

Configuration for the sandbox manager.

#### Default Limits

| Field                      | Type                  | Description                                               |
| -------------------------- | --------------------- | --------------------------------------------------------- |
| `open_files_default_limit` | `MaybeLimited<usize>` | Default limit on the number of opened files               |
| `process_default_limit`    | `MaybeLimited<usize>` | Default process limit for the sandbox                     |
| `memory_default_limit`     | `MaybeLimited<u64>`   | Default memory size limit for the sandbox [Kb]            |
| `stack_default_limit`      | `MaybeLimited<u64>`   | Default stack size limit for the sandbox [Kb]             |
| `time_default_limit`       | `MaybeLimited<f64>`   | Default CPU time limit for the sandbox                    |
| `real_time_default_limit`  | `MaybeLimited<f64>`   | Default real (wall-clock) time limit for the sandbox      |
| `extra_time_default_limit` | `f64`                 | Default extra time allowance after exceeding `time_limit` |

#### `ISOLATE` Rules

| Field             | Type    | Description                                                                                                        | Default               |
| ----------------- | ------- | ------------------------------------------------------------------------------------------------------------------ | --------------------- |
| `sandboxes_count` | `usize` | Maximum number of containers                                                                                       | `1`                   |
| `box_root`        | `Path`  | All sandboxes are created under this directory. This directory and all its ancestors must be writable only by root | `/.invoker/isolate`   |
| `lock_root`       | `Path`  | Directory where lock files are created                                                                             | `/run/isolate/locks`  |
| `cg_root`         | `Path`  | Cgroup root directory                                                                                              | `/run/isolate/cgroup` |
| `first_uid`       | `usize` | First `user_id` reserved for sandboxes                                                                             | `60000`               |
| `first_gid`       | `usize` | First `group_id` reserved for sandboxes                                                                            | `60000`               |
| `restricted_init` | `bool`  | If `true`, only root can create new sandboxes                                                                      | `false`               |

#### Example Configuration

```yaml
sandboxes_count: 1000
process_default_limit: !Limited 1
open_files_default_limit: !Limited 2
memory_default_limit: !Limited 1048576
stack_default_limit: Unlimited
time_default_limit: !Limited 10.0
extra_time_default_limit: 0.0
real_time_default_limit: !Limited 10.0
box_root: /.invoker/isolate
lock_root: /run/isolate/locks
cg_root: /run/isolate/cgroup
first_uid: 60000
first_gid: 60000
restricted_init: false
```

---

### `judge.yaml`

Configuration for judging.

```yaml
compilation_commands:
    python3:
        - "/usr/bin/cp"
        - "--update=none"
        - "$SOURCE"
        - "$OUTPUT"
    g++:
        - "/usr/bin/g++"
        - "-x"
        - "c++"
        - "$SOURCE"
        - "-o"
        - "$OUTPUT"
        - "-O2"
        - "-Wall"
        - "-lm"
```

> [!IMPORTANT]
> Use `-x c++` or equivalent flags to explicitly define the language for the compiler, because source files do not have extensions.

#### `compilation_commands` Reference

| Field     | Type  | Description                          | Default Command                                        |
| --------- | ----- | ------------------------------------ | ------------------------------------------------------ |
| `g++`     | `str` | Compilation command for **C++**      | `/usr/bin/g++ -x c++ $SOURCE -o $OUTPUT -O2 -Wall -lm` |
| `python3` | `str` | Command to prepare **Python** source | `/usr/bin/cp --update=none $SOURCE $OUTPUT`            |

---

## Environment Variables

| Variable                   | Type         | Example              | Description                             |
| -------------------------- | ------------ | -------------------- | --------------------------------------- |
| `INVOKER_MANAGER_HOST`     | `SocketAddr` | `127.0.0.1:5477`     | Address of the manager WebSocket server |
| `INVOKER_CONFIG_DIR`       | `DirPath`    | `.config/invoker`    | Directory for configuration files       |
| `INVOKER_WORK_DIR`         | `DirPath`    | `invoker`            | Working directory for invoker           |
| `INVOKER_ISOLATE_EXE_PATH` | `Path`       | `.local/bin/isolate` | Path to the `isolate` executable        |

---

## API Protocol

Connect via WebSocket client to: `ws://$INVOKER_MANAGER_HOST`

**Message Format:**

```
<stream name>
<message>
```

### Master Stream

#### Incoming Messages

##### Start Task

```
master
TYPE RUN
PACKAGE <id>
LANG G++ / PYTHON3
DATA
<binary data: problem package>
```

##### Stop Task

```
master
TYPE STOP
```

##### Close Invoker

```
master
TYPE CLOSE
```

#### Outgoing Messages

##### Sending Token

```
master
TYPE TOKEN
ID <uuid: token>
NAME <str: name>
```

##### Test Verdict

```
master
TYPE TEST
ID <id>
VERDICT <verdict>
TIME <time>
MEMORY <memory>
DATA
<tar: (output, message)>
```

##### Full Verdict — Success

```
master
TYPE VERDICT
NAME OK
SUM <uint: score>
GROUPS <uint: score group 0> <uint: score group 1> ... <uint: score group n>
```

##### Full Verdict — Compile Error

```
master
TYPE VERDICT
NAME CE
MESSAGE <text: message>
```

##### Full Verdict — Testing Error

```
master
TYPE VERDICT
NAME TE
MESSAGE <text: message>
```

##### Invoker Error

```
master
TYPE ERROR
MESSAGE <text: error message>
```

##### Operator Error

```
master
TYPE OPERROR
MESSAGE <text: error message>
```

##### Exited

```
master
TYPE EXITED
CODE <exit code>
MESSAGE <exit data>
```

---

### Auth Stream

#### Incoming Messages

##### Authenticate Challenge

```
auth
TYPE CHALLENGE
DATA
<bytes: challenge>
```

##### Authenticate Verdict

```
auth
TYPE VERDICT
VERDICT <{APPROVED, DENIED}>
```

#### Outgoing Messages

##### Authenticate Proof

```
auth
TYPE PROOF
DATA
<bytes: proof>
```

---

## Verdicts Reference

| Name | Description           | Is Success |
| ---- | --------------------- | ---------- |
| `OK` | Accepted              | ✅ Yes     |
| `WA` | Wrong Answer          | ❌ No      |
| `TL` | Time Limit Exceeded   | ❌ No      |
| `ML` | Memory Limit Exceeded | ❌ No      |
| `SL` | Stack Limit Exceeded  | ❌ No      |
| `RE` | Runtime Error         | ❌ No      |
| `CE` | Compile Error         | ❌ No      |
| `TE` | Testing System Error  | ❌ No      |

---

## Task Structure

### Directory Layout

```
task_template/
├── config.yaml
├── checker.out / interactive.out
├── [OPTIONAL][type: standard] correct/
│   ├── 1.txt
│   ├── ...
│   └── n.txt
├── [type: standard] input/
│   ├── 1.txt
│   ├── ...
│   └── n.txt
└── [type: interactive] test/
    ├── 1.txt
    ├── ...
    └── n.txt
```

### Checker & Interactor

- Use **Polygon (Codeforces)** standard for checker/interactor implementation.
- ⚠️ Interactive tasks **do not** use a checker.

---

### `config.yaml`

#### Template Example

```yaml
type: standard

limits:
    time: 2
    real_time: 2
    memory: 512000
    stack: 512000 # optional

groups:
    - id: 0
      range: [1, 2]
      cost: 0
      depends: []
    - id: 1
      range: [3, 10]
      cost: 30
      depends: []
    - id: 2
      range: [11, 20]
      cost: 20
      depends: [1]
    - id: 3
      range: [21, 30]
      cost: 50
      depends: []
```

#### Configuration Reference

| Field    | Type         | Description                  |
| -------- | ------------ | ---------------------------- |
| `type`   | `taskType`   | Type of the task             |
| `limits` | `taskLimits` | Resource limits for solution |
| `groups` | `[Group]`    | Test group configurations    |

---

## Type Definitions

### `taskType`

```rust
enum taskType {
    standard,
    interactive
}
```

### `taskLimits`

| Field       | Type    | Description                     |
| ----------- | ------- | ------------------------------- |
| `time`      | `f64`   | CPU time limit [seconds]        |
| `real_time` | `f64`   | Wall-clock time limit [seconds] |
| `memory`    | `usize` | Memory size limit [Kb]          |
| `stack`     | `usize` | Stack size limit [Kb]           |

### `Group`

| Field     | Type          | Description                                                       |
| --------- | ------------- | ----------------------------------------------------------------- |
| `id`      | `GroupId`     | Unique identifier for the group                                   |
| `range`   | `[TestId; 2]` | Inclusive range of test IDs in this group `[first, last]`         |
| `cost`    | `usize`       | Score weight of the group (0 .. 100)                              |
| `depends` | `[GroupId]`   | List of group IDs that must be solved before this group is scored |

### `GroupId`

```rust
type GroupId = usize;
// Numbering starts from 0
```

### `TestId`

```rust
type TestId = usize;
// Test numbering starts from 1
```

---

> 📝 **Notes**
>
> - All time values are in **seconds** (floating point allowed).
> - All memory/stack values are in **kilobytes**.
> - YAML tags like `!Limited` and `Unlimited` are parsed by the configuration loader.
> - Placeholders `$SOURCE` and `$OUTPUT` are substituted at runtime with actual file paths.

_Documentation version: 1.0_
