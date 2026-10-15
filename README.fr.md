# power-sum-graphs

Cycles hamiltoniens des graphes de sommes de puissances : les résultats, leurs
certificats, le logiciel qui les trouve, **posum**, et deux vérificateurs
indépendants.

Pour des entiers `k >= 2` et `n >= 1`, le **graphe de sommes de puissances**
`G_k(n)` a pour sommets `1, ..., n` et relie deux sommets distincts dont la
somme est une puissance `k`-ième parfaite. Un cycle hamiltonien de `G_k(n)`
dispose `1..n` en cercle de sorte que deux voisins aient toujours pour somme
une puissance `k`-ième ; pour `k = 2`, c'est le problème classique des sommes
de carrés. `n_min(k)` est le plus petit `n` pour lequel une telle disposition
existe, suite OEIS [A115418](https://oeis.org/A115418).

[English](README.md)

## Résultats

| k | `n_min(k)` | forme close | statut | bundle |
|---|---|---|---|---|
| 2 | 32 | — | classique ([A071984](https://oeis.org/A071984)) ; cycle ici, minimalité citée | [`k2-n32`](witness/bases/k2-n32/) |
| 3 | **473** | `9^3 - 8^3/2` | trouvé par G. Resta ; minimalité certifiée ici | [`k3-n473`](witness/bases/k3-n473/) |
| 4 | **9641** | `11^4 - 10^4/2` | A115418, R. Tauraso, 2006 ; minimalité certifiée ici | [`k4-n9641`](witness/bases/k4-n9641/) |
| 5 | **246877** | `13^5 - 12^5/2` | nouveau, à notre connaissance | [`k5-n246877`](witness/bases/k5-n246877/) |
| 6 | **15748961** | `17^6 - 16^6/2` | nouveau, à notre connaissance | [`k6-n15748961`](witness/bases/k6-n15748961/) |
| 7 | `>= 587761723` | `19^7 - 18^7/2` | borne inférieure, certifiée ici | — |

Chaque valeur en gras est un théorème : son bundle contient un cycle
hamiltonien **et** un certificat de bandes qui exclut chaque `n` plus petit,
par un sommet de degré au plus un pour chaque plage de `n`. Pour `k = 2`, un tel
certificat n'existe pas : tous les degrés de `G_2(31)` valent au moins deux, et
il n'est pas hamiltonien pour une raison plus fine.

**Îles admissibles.** Un sommet de degré inférieur à deux interdit tout cycle :
`G_k(n)` ne peut être hamiltonien que si tous ses degrés valent au moins deux.
Pour `k >= 4`, les `n` qui le permettent forment des **îles** finies avant une
dernière, non bornée : `9641 - 9999` puis `18193 - ...` pour `k = 4`,
`246877 - 248831` puis `490463 - ...` pour `k = 5` (`posum islands <k>`). Tous
les `n` des îles finies de `k = 4` et `k = 5` sont hamiltoniens, témoins dans
[`k4-n9641-9999`](witness/bases/k4-n9641-9999/) et
[`k5-n246877-248831`](witness/bases/k5-n246877-248831/), ce qui appuie :

> **Conjecture.** Pour `k >= 3`, `G_k(n)` est hamiltonien exactement quand
> tous ses sommets sont de degré au moins deux.

Énoncée avec ses indices ; non revendiquée.

## Vérifier un résultat

Rien ici ne demande de faire confiance à la recherche. Chaque témoin est un
fichier de `n - 1` **bases** : en partant de 1, chaque base `b` donne le sommet
suivant `b^k - v`, et le cycle se referme sur 1. Depuis un clone :

```console
cargo build --release --manifest-path verify/rust/Cargo.toml
xz -dk witness/bases/k5-n246877/25b320b3.bases.xz

verify/rust/target/release/posum-verify bases 5 246877 witness/bases/k5-n246877/25b320b3.bases
python3 verify/python/posum_verify.py bases 5 246877 witness/bases/k5-n246877/25b320b3.bases
python3 verify/python/posum_verify.py bands 5 246877 witness/bases/k5-n246877/bands.txt
```

Les deux vérificateurs ne partagent aucun code avec posum ni entre eux. Celui en
Rust n'a **aucune dépendance**, celui en Python n'utilise que la bibliothèque
standard ; chacun lit un fichier et le confronte à `doc/formats.md` : une
permutation de `1..n` dont les sommes consécutives, fermeture comprise, sont des
puissances `k`-ièmes, ou des bandes couvrant `1..n-1` avec les degrés annoncés.
`contrib/awk/` décode un fichier de bases en liste simple et vérifie les listes
simples, en quelques lignes d'awk.

Les listes simples, un sommet par ligne, sont dérivées des bases (`posum bundle
plain`) et publiées avec chaque release sur Zenodo. L'identité d'un témoin est
le SHA-256 de sa liste simple.

## Régénérer les témoins

Chaque témoin a été trouvé par posum avec LKH-3, et sa provenance dit comment :
binaire, graine, segments, durée. Pour tous les retrouver :

```console
scripts/regenerate.sh            # environ 1 h 20 sur 16 cœurs ; --no-k6 pour sauter G_6
```

Le script construit posum et LKH, balaie les îles, lance les recherches
isolées, range chaque témoin dans `witness-regenerated/`, contrôle le tout avec
les deux vérificateurs et le compare aux bundles publiés. LKH est compilé pour
le processeur local : une autre machine peut trouver d'autres cycles, contrôlés
de la même façon.

`scripts/check.sh` est la porte avant publication : lints, tests, vérificateurs
contre les vecteurs de test, certificats de bandes pour `k = 3..7` et `posum
check` sur chaque bundle.

## posum

```console
posum islands <k>                     îles admissibles de G_k
posum bands <k> <n>                   certificat de borne inférieure pour n
posum search <k> <n>                  cherche un cycle hamiltonien avec LKH
posum sweep <k> island                chaque n de chaque île finie, en parallèle
posum encode | decode                 liste simple <-> bases
posum bundle add | plain              range les témoins, dérive l'arbre simple
posum check                           chaque bundle, les deux vérificateurs
```

Une recherche refuse un `n` hors des îles, propage les contraintes de degré
deux, contracte les chaînes d'arêtes forcées et confie le reste à LKH sous forme
de problème TSPLIB, par segments reprenables sous limite de temps.
`doc/methods.md` décrit les algorithmes, `doc/formats.md` chaque fichier,
`doc/reproduce.md` un chemin par résultat, `doc/design.md` les décisions.

**LKH** est [LKH-3](http://webhotel4.ruc.dk/~keld/research/LKH-3/) de Keld
Helsgaun, distribué pour un usage académique et non commercial. Il ne fait pas
partie de ce dépôt : `scripts/build-lkh.sh` télécharge l'archive officielle,
vérifie son SHA-256 et le compile sur votre machine.

## Historique et crédits

- Giovanni Resta a trouvé le cycle de longueur 473 pour les cubes
  ([Puzzle 311](http://www.primepuzzles.net/puzzles/puzz_311.htm)) ; Roberto
  Tauraso a soumis A115418 avec ses termes jusqu'à `k = 4` en 2006.
- 32 est la réponse classique au problème des sommes de carrés ; dans
  [A071984](https://oeis.org/A071984) (2018), Bert Dobbelaere a noté
  l'argument des degrés pour `n <= 30` et qu'on écarte `n = 31` à la main.
- L'exploration qui a mené à cette release — le moteur SAT qui a trouvé le
  premier `n_min(5)` et `n_min(6)`, les mesures, les échecs, les notes de
  recherche — est le carnet de laboratoire
  [math-cafe/powercycle](https://github.com/math-cafe/powercycle), public, cité
  comme provenance. Ses témoins sont valides et vérifiés indépendamment ; la
  release 1.0 ne publie que des témoins retrouvés par posum.

## Licences et citation

Code : MIT ou Apache-2.0, au choix ([LICENSE](LICENSE)). Témoins : domaine
public, CC0 1.0 ([witness/LICENSE](witness/LICENSE)). Pour citer, voir
[CITATION.cff](CITATION.cff).
