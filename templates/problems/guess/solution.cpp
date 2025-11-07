#pragma GCC optimize("O3,unroll-loops")
#include <bits/stdc++.h>
#pragma GCC target("avx,avx2")

using namespace std;

using ll = long long;

#define all(x) (x).begin(), (x).end()
#ifdef DBG
#define $(x) x
#else
#define $(x)
#endif

#define debug(x) $(cout << #x << " = " << x << endl;)

mt19937 rnd(54);


void WA() {
    cout << "it's prompt for get WA" << endl;
    exit(0);
}

void TL() {
    bool _ = 0;
    while(1) {
        _ = (_ + 11) / 3;
    }
}

void RE() {
    assert(0);
}

constexpr int INF = 1e9;

int query(int x) {
    cout << x << endl;
    char c;
    cin >> c;
    if (c == '<') {
        return -1;
    }
    if (c == '>') {
        return 1;
    }
    exit(0);
}

void solve() {
    int l = 0;
    int r = 1e9;
    while (l + 1 < r) {
        int mid = (l + r) / 2;
        if (query(mid) < 0) {
            l = mid;
        } else {
            r = mid;
        }
    }
}

int main() {
    int t = 1;
    //$(cin >> t;)

    while (t--) {
        solve();
        $(cout << endl;)
    }
}
