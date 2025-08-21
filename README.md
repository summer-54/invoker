# Invoker
## Incoming
### Start task
```
START
<binary data>
```
### Stop task
```
STOP
```
### Close invoker
```
CLOSE
```
## Outgoing
### Full verdict
```
VERDICT OK
SUM <score>
<score group 0>
<score group 1>
<score group 2>
<score group 3>
...
<score group n>
```
or
```
VERDICT CE
<message>
```
or
```
VERDICT TE
<message>
```

### Test verdict
```
TEST <id>
VERDICT <verdict>
TIME <time>
MEMORY <memory>
<data: tar: (output, message)>
```
### Exited
```
EXITED <exit code>
<exit data>
```
### Invoker error
```
ERROR
<error message>
```
### Operator error
```
OPERROR
<error message>
```
### Sending token
```
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
