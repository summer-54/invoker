cd "$(dirname "$0")"
cd $NAME
g++ checker.cpp -o checker.out
CORRECT=""
if test -d "correct"; then CORRECT="correct"; fi
tar -cf ../archives/$NAME.tar input $CORRECT checker.out config.yaml solution
