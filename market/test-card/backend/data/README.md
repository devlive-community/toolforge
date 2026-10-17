# Chinese credit-card BIN reference snapshot

The 54 six-digit prefixes in `src/bank.rs` are a small, offline subset of the
following MIT-licensed datasets, checked on 2026-10-16:

- [`hexindai/bcbc`, `data/bin.csv`](https://github.com/hexindai/bcbc/blob/de631827ffe8db2792d140f3476b02498fc1244f/data/bin.csv),
  revision `de631827ffe8db2792d140f3476b02498fc1244f`.
  Copyright (c) 2018–2020 Runrioter; see `bcbc-LICENSE`.
- [`caijf/bankcard`, `src/data/bin.ts`](https://github.com/caijf/bankcard/blob/79af3e5d1bd381537c2f0e95c03584c911963d1e/src/data/bin.ts),
  revision `79af3e5d1bd381537c2f0e95c03584c911963d1e`.
  Copyright (c) 2019 caijf; see `bankcard-LICENSE`.

Retained rows must agree on bank, `CC` (credit), and length 16 in both sources,
start with `62`, and have no conflicting bank assignment from a shorter or longer
prefix in either snapshot. Each of the 18 banks has at most three retained BINs.
Debit rows such as CMB `622588` are excluded. The sources may share an upstream
dataset, so their agreement is not independent confirmation by a bank.

Some retained prefixes also appear in bank publications:

- [China Construction Bank](https://www1.ccb.com/sn/cn/faverable/20160837_1425457621.html):
  `622168`, `622708`, `625966`, UnionPay IC credit cards.
- [China Merchants Bank](https://market.cmbchina.com/MPage/online/220722093008825/hkbx/hkbx.html):
  `622575`, `622576`, corporate credit cards.
- [Agricultural Bank of China](https://www.abchina.com/zt/branch/ha/preInfo/fwgg/201401/t20140122_862147.htm):
  `622836`, `622837`, 16-digit credit cards.

Generated numbers combine one reference prefix, random digits, and a Luhn check
digit. They are local format fixtures, not issued cards, bank-backed credentials,
or payment-platform sandbox cards. Reference matching in the validator does not
confirm an issuer, an account, or a card's status. This bounded snapshot is not a
complete or current BIN directory and does not claim eight-digit IIN accuracy.

Both license notices are embedded by `include_str!` in the `banks` response and
bank-specific JSON exports, so the packaged WASM plugin retains the notices even
though the package builder does not copy this directory separately.
