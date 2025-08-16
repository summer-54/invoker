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
VERDICT <score>
GROUPS
0
20
70
```
### Test verdict
```
TEST <id>
VERDICT <verdict>
TIME 12
MEMORY 1123
<data: tar: (output, checker_output)>
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
 - WA: wrong answer
 - CE: compile error
 - RE: runtime error
 - SK: skipped
 - OK: ok
