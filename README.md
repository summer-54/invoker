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
### Stop task
```
CLOSE
```
## Outgoing
### Full task verdict
```
VERDICT <verdict>
<data>
```
### Test verdict
```
SUBTASK <subtask id>
VERDICT <verdict>
<data>
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
