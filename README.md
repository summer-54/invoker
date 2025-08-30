# Invoker
## Incoming
### Start task
```
TYPE START
DATA
<binary data: tar.gz: problem_template>
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
VERDICT OK
SUM <score>
GROUPS <score group 0> <score group 1> ... <score group n>
```
or
```
TYPE VERDICT
VERDICT CE
MESSAGE <message>
```
or
```
TYPE VERDICT
VERDICT TE
MESSAGE <message>
```

### Test verdict
```
TYPE TEST
TEST <id>
VERDICT <verdict>
TIME <time>
MEMORY <memory>
DATA
<data: tar.gz: (output, message)>
```
### Exited
```
TYPE EXITED
EXITED <exit code>
DATA
<exit data>
```
### Invoker error
```
TYPE ERROR
ERROR <error message>
```
### Operator error
```
TYPE OPERROR
OPERROR <error message>
```
### Sending token
```
TYPE TOKEN
TOKEN <token: uuid>
```

## Verdicts:
 - OK  //ok
 - WA, //wrong answer
 - ML, //memory limit
 - TL, //time limit
 - RE, //runtime error
 - CE, //compile error
 - TE, //testing system error
 - SL, //stack limit
