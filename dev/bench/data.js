window.BENCHMARK_DATA = {
  "lastUpdate": 1773683707761,
  "repoUrl": "https://github.com/jburrow/fast_code_search",
  "entries": {
    "fast_code_search Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "jaburrow@gmail.com",
            "name": "James Burrow",
            "username": "jburrow"
          },
          "committer": {
            "email": "jaburrow@gmail.com",
            "name": "James Burrow",
            "username": "jburrow"
          },
          "distinct": true,
          "id": "73a09184c1fab1ed256a21243bf14634fdb0c331",
          "message": "Add theme toggle, version injection, and dynamic changelog functionality\n\n- Implemented a theme toggle feature for light and dark modes.\n- Added version injection from version.json for display in the footer.\n- Created a fallback mechanism for the benchmark iframe if the page is not available.\n- Integrated dynamic changelog loading from changelog.json to replace static entries.\n- Enhanced navigation with smooth active highlighting based on section visibility.\n- Introduced new CSS styles for improved layout and responsiveness across various components.",
          "timestamp": "2026-03-02T07:40:40Z",
          "tree_id": "c68d3471f2a283cb520d6376bb78cccd01e07df2",
          "url": "https://github.com/jburrow/fast_code_search/commit/73a09184c1fab1ed256a21243bf14634fdb0c331"
        },
        "date": 1772438127712,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 299254,
            "range": "± 23630",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22958,
            "range": "± 309",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 482,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 509130,
            "range": "± 13194",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 23414,
            "range": "± 374",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 646,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 923390,
            "range": "± 23744",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 24128,
            "range": "± 535",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 959,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 380408,
            "range": "± 19983",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 614303,
            "range": "± 24458",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 538563,
            "range": "± 18023",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 829802,
            "range": "± 7566",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 511739,
            "range": "± 33778",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 350440,
            "range": "± 5558",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 525187,
            "range": "± 5960",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 698990,
            "range": "± 11123",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 506689,
            "range": "± 22238",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 512566,
            "range": "± 24304",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 250420,
            "range": "± 21876",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 506390,
            "range": "± 35844",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 513859,
            "range": "± 39207",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 502595,
            "range": "± 21180",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 324296,
            "range": "± 17986",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 270026,
            "range": "± 11282",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3672,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18076422,
            "range": "± 273636",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 35616684,
            "range": "± 127034",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 71003298,
            "range": "± 593165",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31229419,
            "range": "± 112394",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32086799,
            "range": "± 126782",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 61595136,
            "range": "± 693395",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 64373999,
            "range": "± 556020",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 894724,
            "range": "± 33802",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3384120,
            "range": "± 50810",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6598447,
            "range": "± 312875",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1967823,
            "range": "± 16240",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8085207,
            "range": "± 143403",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16746526,
            "range": "± 413714",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 201703,
            "range": "± 18399",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 307898,
            "range": "± 8862",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 464466,
            "range": "± 4524",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 149207,
            "range": "± 3189",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 673237,
            "range": "± 4367",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1331959,
            "range": "± 28357",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jaburrow@gmail.com",
            "name": "James Burrow",
            "username": "jburrow"
          },
          "committer": {
            "email": "jaburrow@gmail.com",
            "name": "James Burrow",
            "username": "jburrow"
          },
          "distinct": true,
          "id": "451c2f49f717baf4d0b5bef88e525231ebe819aa",
          "message": "fix: use fetch probe for benchmark iframe (404 detection)",
          "timestamp": "2026-03-02T07:49:27Z",
          "tree_id": "232443aa50c312573f7c21b832552b815c1a69de",
          "url": "https://github.com/jburrow/fast_code_search/commit/451c2f49f717baf4d0b5bef88e525231ebe819aa"
        },
        "date": 1772438881467,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 294325,
            "range": "± 57229",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 21943,
            "range": "± 286",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 483,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 510644,
            "range": "± 29705",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 22037,
            "range": "± 266",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 638,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 896242,
            "range": "± 21853",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 22775,
            "range": "± 290",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 963,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 386135,
            "range": "± 42038",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 579405,
            "range": "± 27044",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 515230,
            "range": "± 38190",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 825389,
            "range": "± 4024",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 497463,
            "range": "± 25837",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 347459,
            "range": "± 4970",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 514232,
            "range": "± 8886",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 684327,
            "range": "± 4322",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 497742,
            "range": "± 34564",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 502745,
            "range": "± 21947",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 252306,
            "range": "± 20632",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 492849,
            "range": "± 19585",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 493427,
            "range": "± 24364",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 493887,
            "range": "± 20728",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 319234,
            "range": "± 10654",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 267574,
            "range": "± 13527",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3729,
            "range": "± 796",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18239453,
            "range": "± 58819",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36181382,
            "range": "± 147577",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 71701477,
            "range": "± 281058",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31635895,
            "range": "± 104115",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32652392,
            "range": "± 133708",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63077565,
            "range": "± 336026",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65837818,
            "range": "± 148555",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 873327,
            "range": "± 19840",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3414000,
            "range": "± 139382",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6404389,
            "range": "± 128850",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1988257,
            "range": "± 8412",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8104041,
            "range": "± 89235",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16292083,
            "range": "± 77755",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 200330,
            "range": "± 2730",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 308219,
            "range": "± 22423",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 463499,
            "range": "± 5752",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147905,
            "range": "± 2221",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 670765,
            "range": "± 5495",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1320052,
            "range": "± 34288",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "198982749+Copilot@users.noreply.github.com",
            "name": "Copilot",
            "username": "Copilot"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "decc1354cec738f0deec72c5e5dee4854cbc9534",
          "message": "fix: preserve dev/bench/ across GitHub Pages deployments (#83)\n\n* Initial plan\n\n* fix: preserve dev/bench/ benchmark data across GitHub Pages deployments\n\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>\n\n---------\n\nCo-authored-by: copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>",
          "timestamp": "2026-03-03T07:27:53Z",
          "tree_id": "0d51686de357951acadaa1c4d65c72ff9960f92a",
          "url": "https://github.com/jburrow/fast_code_search/commit/decc1354cec738f0deec72c5e5dee4854cbc9534"
        },
        "date": 1772523574958,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 301432,
            "range": "± 67047",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22157,
            "range": "± 377",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 482,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 504050,
            "range": "± 19291",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 22424,
            "range": "± 287",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 646,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 914668,
            "range": "± 30971",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 22995,
            "range": "± 340",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 959,
            "range": "± 50",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 366638,
            "range": "± 15056",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 598112,
            "range": "± 28033",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 525454,
            "range": "± 16634",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 830423,
            "range": "± 44104",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 502946,
            "range": "± 18175",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 350352,
            "range": "± 5078",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 529985,
            "range": "± 27387",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 693181,
            "range": "± 6842",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 496934,
            "range": "± 18904",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 508512,
            "range": "± 20598",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 252516,
            "range": "± 23496",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 494871,
            "range": "± 23000",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 500108,
            "range": "± 29598",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 496533,
            "range": "± 23438",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 322410,
            "range": "± 10776",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 273462,
            "range": "± 14180",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3708,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 17864524,
            "range": "± 43907",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 35571178,
            "range": "± 83389",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 70944833,
            "range": "± 313566",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 30961483,
            "range": "± 58723",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 31884613,
            "range": "± 98599",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 61390735,
            "range": "± 260986",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 64071315,
            "range": "± 152184",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 897012,
            "range": "± 24289",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3381699,
            "range": "± 31229",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6413311,
            "range": "± 197891",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1976968,
            "range": "± 18524",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8093758,
            "range": "± 25107",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16220440,
            "range": "± 91427",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 195864,
            "range": "± 2628",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 304501,
            "range": "± 10678",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 461183,
            "range": "± 5720",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 148317,
            "range": "± 2936",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 673157,
            "range": "± 7750",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1332438,
            "range": "± 26968",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jaburrow@gmail.com",
            "name": "James Burrow",
            "username": "jburrow"
          },
          "committer": {
            "email": "jaburrow@gmail.com",
            "name": "James Burrow",
            "username": "jburrow"
          },
          "distinct": true,
          "id": "7bc2d9c7e82f23c351e896f3911ae8090a95d3ff",
          "message": "Merge branch 'copilot/investigate-fast-code-search-404' into main",
          "timestamp": "2026-03-03T08:03:22Z",
          "tree_id": "226368283c94479b6cb26191ffe4a941ecdd774d",
          "url": "https://github.com/jburrow/fast_code_search/commit/7bc2d9c7e82f23c351e896f3911ae8090a95d3ff"
        },
        "date": 1772525705189,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 307579,
            "range": "± 54469",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 19434,
            "range": "± 255",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 396,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 556530,
            "range": "± 10430",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 19140,
            "range": "± 218",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 504,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 1028379,
            "range": "± 24435",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 20568,
            "range": "± 307",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 702,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 415140,
            "range": "± 13891",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 711894,
            "range": "± 21462",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 576618,
            "range": "± 14954",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 876757,
            "range": "± 2456",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 551927,
            "range": "± 11856",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 292854,
            "range": "± 3615",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 539194,
            "range": "± 2720",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 710441,
            "range": "± 2827",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 554090,
            "range": "± 13895",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 556340,
            "range": "± 12503",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 255763,
            "range": "± 17545",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 551225,
            "range": "± 7823",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 552483,
            "range": "± 13828",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 548007,
            "range": "± 10931",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 348869,
            "range": "± 6819",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 296765,
            "range": "± 10389",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3332,
            "range": "± 39",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 16476199,
            "range": "± 38854",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 32859428,
            "range": "± 83004",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 64865310,
            "range": "± 102944",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 28047126,
            "range": "± 42109",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 28748631,
            "range": "± 61814",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 55577098,
            "range": "± 150980",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 58124754,
            "range": "± 170031",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 643767,
            "range": "± 13257",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 2475698,
            "range": "± 11335",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 4671194,
            "range": "± 27680",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1310810,
            "range": "± 18713",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 5230793,
            "range": "± 68291",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 10556537,
            "range": "± 96701",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 201048,
            "range": "± 5522",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 316745,
            "range": "± 7109",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 499723,
            "range": "± 3993",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 80991,
            "range": "± 1358",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 356109,
            "range": "± 26025",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 699705,
            "range": "± 5344",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "jaburrow@gmail.com",
            "name": "James Burrow",
            "username": "jburrow"
          },
          "committer": {
            "email": "jaburrow@gmail.com",
            "name": "James Burrow",
            "username": "jburrow"
          },
          "distinct": true,
          "id": "6e03bb975137ff452face35d25871cdc4b83f2d2",
          "message": "feat: add VS Code configuration and tasks for Rust development",
          "timestamp": "2026-03-16T17:41:23Z",
          "tree_id": "8ba5afce0921ddf8efa5ccd2255dfa41950006b6",
          "url": "https://github.com/jburrow/fast_code_search/commit/6e03bb975137ff452face35d25871cdc4b83f2d2"
        },
        "date": 1773683706712,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 301876,
            "range": "± 13215",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22810,
            "range": "± 680",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 488,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 508045,
            "range": "± 17382",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 23381,
            "range": "± 589",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 648,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 915913,
            "range": "± 34641",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 24490,
            "range": "± 398",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 960,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 368111,
            "range": "± 10796",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 602768,
            "range": "± 21798",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 523854,
            "range": "± 15818",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 830096,
            "range": "± 7967",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 510309,
            "range": "± 43601",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 354761,
            "range": "± 5369",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 525877,
            "range": "± 3641",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 694958,
            "range": "± 6353",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 507168,
            "range": "± 19720",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 516165,
            "range": "± 13536",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 249037,
            "range": "± 20808",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 510342,
            "range": "± 11672",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 512367,
            "range": "± 16862",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 507073,
            "range": "± 35397",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 335686,
            "range": "± 10499",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 283258,
            "range": "± 23643",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3701,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18383348,
            "range": "± 72024",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36508702,
            "range": "± 94275",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 72597188,
            "range": "± 1087261",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31804892,
            "range": "± 99201",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32717437,
            "range": "± 75382",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63212588,
            "range": "± 233063",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65933050,
            "range": "± 179046",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 881051,
            "range": "± 31216",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3386334,
            "range": "± 43926",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6401986,
            "range": "± 239457",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1957654,
            "range": "± 14040",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7893744,
            "range": "± 31429",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 15886149,
            "range": "± 42058",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 200817,
            "range": "± 6132",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 314153,
            "range": "± 15500",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 474573,
            "range": "± 16236",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 148549,
            "range": "± 3034",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 671677,
            "range": "± 9051",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1327498,
            "range": "± 81887",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}