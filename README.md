# Invoker
## Incoming
### Start task
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
 - OK  //ok
 - WA, //wrong answer
 - ML, //memory limit
 - TL, //time limit
 - RE, //runtime error
 - CE, //compile error
 - TE, //testing system error
 - SL, //stack limit
