# design_record: leaf report (v0)

## Citations checked

Public pages only (Europe PMC records, publisher metadata, SAS and statsmodels-side documentation,
the authors' slides); no download. The full texts of the papers were not available, so a check
confirms the bibliographic record and the abstract-level claim; where a claim needs the body text it
says so. No source contradicts the battery.

| Citation | Check | Result |
|:--|:--|:--|
| Lydersen, Fagerland, Laake 2009, Stat Med 28(7):1159-1175, doi:10.1002/sim.3531 | Europe PMC record and abstract: title, authors, pages match; the abstract recommends Fisher's test with mid-p adjustment (about as good as an unconditional test) and says the traditional test should practically never be used | held. The two-sided tie rule (half weight on tables equal to the observed) is not worded in the abstract; the authors' slides compute the mid-p as the exact p minus half the observed table's probability, which agrees. Body text not read |
| Agresti, Categorical Data Analysis, exact inference in 2x2xK tables | a book: no public page | not checked; architecture.md cites it without a section number |
| Mantel & Haenszel 1959, JNCI 22(4):719-748 | publisher and index records | held (volume, pages, content: stratified chi-square and the pooled relative-risk estimator) |
| Robins, Breslow, Greenland 1986, Biometrics 42:311-323 | index records and abstract | held. SAS documentation attributes the MH odds-ratio variance to Greenland & Robins 1985; RBG 1986 is the estimator consistent in both limits and the one statsmodels' `StratifiedTable` implements, which the vectors match. Not a contradiction |
| Mantel & Fleiss 1980, Am J Epidemiol 112:129 | Europe PMC record (PMID 7395847) | **fixed**: the title is "Minimum expected cell size requirements for the Mantel-Haenszel one-degree-of-freedom chi-square test and a related rapid procedure", pages 129-134, doi:10.1093/oxfordjournals.aje.a112962; the criterion (min of the gaps from the expected sum to the lower and upper bound sums, accepted at 5 or more) is as in the battery, confirmed from SAS documentation and a second source. A 1998 simulation (Chung & Davis) found the criterion imperfect: the nominal size was exceeded in about a fifth of cases that met it. The battery's fallback and "never a gatekeeper" for homogeneity are consistent with that |
| Breslow & Day 1980, vol. I, IARC Sci Publ 32 | index records | held (pp. 5-338). The test needs sizeable strata, and Candler reports it as information only |
| Phipson & Smyth 2010, SAGMB 9(1): article 39, doi:10.2202/1544-6115.1585 | Europe PMC record and abstract | held at abstract level. The abstract does not print (b+1)/(m+1); secondary descriptions give it as counting the observed labelling among the permutations. The leaf report's "section" pointers are not confirmed, so architecture.md names none. The authors' own exact p (`permp`) refines this for sampling with replacement; Candler uses the simple form |
| Efron 1987, JASA 82(397):171-185, doi:10.1080/01621459.1987.10478410 | index records | held, with one page range (171-200) in one R manual treated as a typo. The equation numbers (2.3, 6.6) in the resampling report are **not confirmed** and are not cited in architecture.md; the jackknife acceleration is described, not numbered |
| Good 2005 ch. 3; Pesarin 2001 ch. 2 (restricted permutation) | books | not checked; not cited in architecture.md (exchangeability is argued from the design) |

Tally: held 6, fixed 1 (Mantel-Fleiss title and pages), contradicting 0, not checkable 3.

## Commits

See `git log afd32f0b..HEAD`.
