#include "testlib.h"
#include <bits/stdc++.h>
#include <cstdio>

using namespace std;

const int INF = 1000'000'000;

int main(int argc, char* argv[]) {
    registerInteraction(argc, argv);
    int x = inf.readInt();
    int q;

    while (cin >> q) {
        tout << q << endl;
        if (q < x) {
            cout << '<' << endl;
        } else if (q > x) {
            cout << '>' << endl;
        } else {
            cout << '=' << endl;
            quitf(_ok, "it's right answer");
        }
    }
    quitf(_wa, "it's not right answer");
}
