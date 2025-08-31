# Configs
## `isolate.yaml`

Config for sandbox manager

### Default limits

| Field                      | Type                  | Description                                                     |
| -------------------------- | --------------------- | --------------------------------------------------------------- |
| `process_default_limit`    | `MaybeLimited<usize>` | Default process limit sandbox                                   |
| `stack_default_limit`      | `MaybeLimited<usize>` | Default stack size limit sandbox [Kb]                           |
| `extra_time_default_limit` | `f64`                 | Default extra time limit sandbox (after exceeding `time_limit`) |
| `open_files_default_limit` | `MaybeLimited<usize>` | Default limit number opened files                               |
### `ISOLATE` rules

| Field             | Type    | Description                                                                                                         | Default               |
| ----------------- | ------- | ------------------------------------------------------------------------------------------------------------------- | --------------------- |
| `sandboxes_count` | `usize` | Maximum number of containers                                                                                        | `1`                   |
| `box_root`        | `str`   | All sandboxes are created under this directory. This directory and all its ancestors must be writeable only to root | `/.invoker/isolate`   |
| `lock_root`       | `str`   | Directory where lock files are created                                                                              | `/run/isolate/locks`  |
| `cg_root`         | `str`   | -                                                                                                                   | `/run/isolate/cgroup` |
| `first_uid`       | `usize` | First `user_id` reserved for sandboxes                                                                              | `60000`               |
| `first_gid`       | `usize` | First `group_id` reserved for sandbox                                                                               | `60000`               |
| `restricted_init` | `bool`  | Only root can create new sandboxes                                                                                  | `false`               |
``` yaml
sandboxes_count: 1000
process_default_limit: !Limited 1
stack_default_limit: Unlimited
extra_time_default_limit: 0.0
open_files_default_limit: !Limited 2
box_root: /.invoker/isolate
lock_root: /run/isolate/locks
cg_root: /run/isolate/cgroup
first_uid: 60000
first_gid: 60000
restricted_init: false
```
# Enviroment variables

- `INVOKER_MANAGER_HOST: SocketAddr` for example  `127.0.0.1:5477`
- `INVOKER_CONFIG_DIR: DirPath` for example `.config/invoker`
- `INVOKER_WORK_DIR: DirPath`  for example `invoker`
# Api
## Incoming

### Websocket client at `ws://$INVOKER_MANAGER_HOST`


#### Start task
```
TYPE START
DATA
<binary data: tar: problem_template>
```
### Stop task
```
TYPE STOP
```
### Close invoker
```
TYPE CLOSE
```
## Outgoing
### Full verdict
```
TYPE VERDICT
NAME OK
SUM <score>
GROUPS <score group 0> <score group 1> ... <score group n>
```
or
```
TYPE VERDICT
NAME CE
MESSAGE <message>
```
or
```
TYPE VERDICT
NAME TE
MESSAGE <message>
```

### Test verdict
```
TYPE TEST
ID <id>
VERDICT <verdict>
TIME <time>
MEMORY <memory>
DATA
<data: tar: (output, message)>
```
### Exited
```
TYPE EXITED
CODE <exit code>
MESSAGE <exit data>
```
### Invoker error
```
TYPE ERROR
MESSAGE <error message>
```
### Operator error
```
TYPE OPERROR
MESSAGE <error message>
```
### Sending token
```
TYPE TOKEN
ID <token: uuid>
```

## Verdicts:

| Name | Descryption           | Is success |
| ---- | --------------------- | ---------- |
| OK   | ok                    | yes        |
| WA   | wrong answer          | no         |
| TL   | time limit exceeded   | no         |
| ML   | memory limit exceeded | no         |
| SL   | stack limit exeeded   | no         |
| RE   | runtime error         | no         |
| CE   | compile error         | no         |
| TE   | testing system error  | no         |
# Problems
``` files
templates/problem_template
├── config.yaml
├── checker.out
├── correct
│   ├── 1.txt
│   │   ...
│   └── n.txt
├── [OPTION] intput
│   ├── 1.txt
│   │   ...
│   └── n.txt
└── solution
```

## `config.yaml`
### Template
``` yaml
type: standart

lang: g++

limits:
  time: 2000
  real_time: 2000
  memory: 512000

  stack: 512000 #optionally

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
    depends:
```
### Config

| Field    | Type            | Description          |
| -------- | --------------- | -------------------- |
| `type`   | `ProblemType`   | Problem type         |
| `lang`   | `Lang`          | Compiler name        |
| `limits` | `ProblemLimits` | Limits for solutiuon |
| `groups` | \[Group]        | Groups configs       |
## `ProblemType`
- `standart`
## `Lang`
- `g++`
## `ProblemLimits`

| Field       | Type    | Description             |
| ----------- | ------- | ----------------------- |
| `time`      | `f64`   | Time limit \[seconds]   |
| `real_time` | `f64`   | Real limit \[seconds]   |
| `memory`    | `usize` | Memory size limit \[Kb] |
| `stack`     | `usize` | Stack size limit \[Kb]  |

## `Group`

| Field     | Type          | Description                                                        |
| --------- | ------------- | ------------------------------------------------------------------ |
| `id`      | `GroupId`     | Time limit \[seconds]                                              |
| `range`   | `[TestId; 2]` | Range tests in this group \[two numbers: \[first, last] inclusive] |
| `cost`    | `usize`       | Cost of group (0 .. 100)                                           |
| `depends` | `[GroupId]`   | List depends groups                                                |

### `GroupId`
`usize`
Numbering starts from __zero__!!!

### `TestId`
`usize`
Numbering starts with __one__!!!
