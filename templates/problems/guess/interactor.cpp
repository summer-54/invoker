#include "testlib.h"
#include <bits/stdc++.h>

using namespace std;

void upd(int& lf, int& rg, int x, int y) {
	if(x > y)
		return;
	lf = max(lf, x);
	rg = min(rg, y);
}

void send(string x) {
	cout << x << endl;
   	fflush(stdout);
}

const int INF = 1000'000'000;

int main(int argc, char* argv[]) {
    registerInteraction(argc, argv);
    int x = inf.readInt();
    tout << x << endl;
    cout << x << endl << flush;
   	int y = ouf.readInt();
    if (x == y) {
        quitf(_ok, "it's right answer");
    } else {
        quitf(_wa, "it's not right answer");
    }
}
