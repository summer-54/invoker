# Invoker
## Incoming
### Start task
```
<task id>
START
<binary data>
```
### Stop task
```
STOP
```
## Outgoing
### Full task verdict
```
VERDICT <verdict>
```
### Test verdict
```
SUBTASK <subtask id>
VERDICT <verdict>
<data>
```
### Exited
```
EXITED <verdict>
```
### Invoker error
```
<task id>
ERROR
<error message>
```
### Operator error
```
<task id>
OPERROR
<error message>
```
