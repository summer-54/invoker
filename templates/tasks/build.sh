#!/usr/bin/bash
cd "$(dirname "$0")"
cd $1
if test -e "checker.cpp"; then g++ checker.cpp -o checker.out; fi
if test -e "interactor.cpp"; then g++ interactor.cpp -o interactor.out; fi

if test -e "solution.cpp"; then cp solution.cpp ../solutions/$1; fi
if test -e "solution.py"; then cp solution.py ../solutions/$1; fi

CORRECT=""
if test -d "correct"; then CORRECT="correct"; fi
INPUT=""
if test -d "input"; then INPUT="input"; fi
CHECKER=""
if test -e "checker.out"; then CHECKER="checker.out"; fi
INTERACTOR=""
if test -e "interactor.out"; then INTERACTOR="interactor.out"; fi
TEST=""
if test -d "test"; then TEST="test"; fi

echo $PWD
tar -cf ../archives/$1.tar $INPUT $CORRECT $CHECKER $INTERACTOR $TEST config.yaml
