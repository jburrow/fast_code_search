window.BENCHMARK_DATA = {
  "lastUpdate": 1773992543164,
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
          "id": "b5cc1fefb58d289124b935af71ee03b84d189c33",
          "message": "Fix: semantic web API handlers panic on poisoned RwLock (#85)\n\n* Initial plan\n\n* Fix: handle RwLock poisoning gracefully in semantic_web API handlers\n\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>\n\n---------\n\nCo-authored-by: copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>",
          "timestamp": "2026-03-17T09:11:52Z",
          "tree_id": "6922bfd02e03d4fa59058dffe0bd57b89004d9b0",
          "url": "https://github.com/jburrow/fast_code_search/commit/b5cc1fefb58d289124b935af71ee03b84d189c33"
        },
        "date": 1773739340835,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 288478,
            "range": "± 11960",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22250,
            "range": "± 433",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 480,
            "range": "± 12",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 509428,
            "range": "± 24686",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 22629,
            "range": "± 456",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 587,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 926548,
            "range": "± 22438",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 23196,
            "range": "± 281",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 960,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 349095,
            "range": "± 21066",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 596304,
            "range": "± 22443",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 506816,
            "range": "± 21850",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 819094,
            "range": "± 10294",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 511113,
            "range": "± 20474",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 347086,
            "range": "± 3578",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 515207,
            "range": "± 19587",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 689751,
            "range": "± 7050",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 502781,
            "range": "± 22850",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 511629,
            "range": "± 22859",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 252536,
            "range": "± 18224",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 503950,
            "range": "± 23532",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 503570,
            "range": "± 33970",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 531794,
            "range": "± 41590",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 328470,
            "range": "± 10806",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 282016,
            "range": "± 16720",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3699,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 17954462,
            "range": "± 79618",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 35592635,
            "range": "± 145126",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 70509661,
            "range": "± 310542",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31094474,
            "range": "± 99668",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 31944315,
            "range": "± 89243",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 61396418,
            "range": "± 291943",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 63898773,
            "range": "± 169808",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 891516,
            "range": "± 13665",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3386494,
            "range": "± 30519",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6434506,
            "range": "± 130129",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 2002810,
            "range": "± 13131",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8107484,
            "range": "± 85165",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16173195,
            "range": "± 147960",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 196296,
            "range": "± 3999",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 312221,
            "range": "± 11378",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 468147,
            "range": "± 11117",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 148538,
            "range": "± 2665",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 671914,
            "range": "± 10173",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1332335,
            "range": "± 24219",
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
          "id": "15bd6632f6849f21ab5cdb54dbc3355e72cb1d7a",
          "message": "Fix path handling: canonical storage, root-relative display, proper glob exclusions (#91)\n\n* Initial plan\n\n* fix: path handling is broken on Windows and inconsistent throughout the stack\n\n- lazy_file_store.rs: Store canonical path in LazyMappedFile.path, not the\n  original non-canonical path. This fixes the strip_prefix mismatch on Windows\n  where canonicalize() adds a UNC prefix.\n\n- engine.rs: Add root_paths field, add_root_path() and make_display_path()\n  methods. make_display_path() strips the canonical root prefix and normalises\n  slashes to forward slashes. All search result file_path fields and\n  get_file_path() now return root-relative display paths.\n\n- engine.rs: Path filter calls use filter_documents_by_display() with\n  root-relative paths so patterns like src/**/*.rs match correctly against\n  display paths.\n\n- path_filter.rs: Add filter_documents_by_display() method that accepts a\n  string-returning closure for pre-computed display paths.\n\n- background_indexer.rs: Call add_root_path() for every configured path at\n  indexer startup so that root-relative display paths are available immediately.\n\n- web/api.rs (file_handler): Use make_display_path() to return root-relative\n  path in FileResponse, consistent with search results.\n\n- file_discovery.rs: Replace substring contains() exclusion check with proper\n  PathFilter glob matching for consistent semantics between discovery-time and\n  search-time exclusions.\n\n- static/index.html: Update filter labels to 'Search only in (glob):' /\n  'Skip paths (glob):' with improved help text clarifying root-relative matching.\n\n- static/keyword.js: Add title attribute to .result-path element so the full\n  path appears on hover.\n\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>\n\n---------\n\nCo-authored-by: copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>",
          "timestamp": "2026-03-17T09:40:30Z",
          "tree_id": "941e545bb7fa80d93cd427b4ea041010bda222db",
          "url": "https://github.com/jburrow/fast_code_search/commit/15bd6632f6849f21ab5cdb54dbc3355e72cb1d7a"
        },
        "date": 1773741079266,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 309325,
            "range": "± 11940",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 23048,
            "range": "± 239",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 512,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 528750,
            "range": "± 26930",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 23155,
            "range": "± 339",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 675,
            "range": "± 18",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 998056,
            "range": "± 21662",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 24034,
            "range": "± 200",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 1018,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 360988,
            "range": "± 20936",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 615189,
            "range": "± 49246",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 522917,
            "range": "± 20174",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 838818,
            "range": "± 5953",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 521083,
            "range": "± 15988",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 355560,
            "range": "± 2292",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 548719,
            "range": "± 11407",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 715607,
            "range": "± 15780",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 525993,
            "range": "± 12017",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 537741,
            "range": "± 19407",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 271522,
            "range": "± 18834",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 514295,
            "range": "± 31235",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 510430,
            "range": "± 21552",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 517028,
            "range": "± 29955",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 348559,
            "range": "± 24157",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 295431,
            "range": "± 10984",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3999,
            "range": "± 32",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18390133,
            "range": "± 332869",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36424089,
            "range": "± 151711",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 72247806,
            "range": "± 288289",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31628400,
            "range": "± 84520",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32544621,
            "range": "± 119613",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 62731452,
            "range": "± 364625",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65494532,
            "range": "± 406883",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 910745,
            "range": "± 58531",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3501025,
            "range": "± 60762",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6709765,
            "range": "± 124853",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1955268,
            "range": "± 21753",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7961855,
            "range": "± 115720",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16342363,
            "range": "± 199679",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 198105,
            "range": "± 3439",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 313708,
            "range": "± 12652",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 473034,
            "range": "± 11945",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 148887,
            "range": "± 3637",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 673666,
            "range": "± 12939",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1326830,
            "range": "± 45614",
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
          "id": "ba1a4a98b6c293ba872d25fc12e1d4d87e11d283",
          "message": "Refactor code structure for improved readability and maintainability",
          "timestamp": "2026-03-17T09:59:01Z",
          "tree_id": "3e2a9e0f80246533983972f34a06c969946f2204",
          "url": "https://github.com/jburrow/fast_code_search/commit/ba1a4a98b6c293ba872d25fc12e1d4d87e11d283"
        },
        "date": 1773742184790,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 315972,
            "range": "± 16956",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 19539,
            "range": "± 723",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 406,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 582956,
            "range": "± 26266",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 19851,
            "range": "± 819",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 499,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 1073096,
            "range": "± 16963",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 21332,
            "range": "± 845",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 707,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 459651,
            "range": "± 14013",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 742183,
            "range": "± 25315",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 633215,
            "range": "± 18360",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 936230,
            "range": "± 15912",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 569487,
            "range": "± 40998",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 304861,
            "range": "± 3873",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 568714,
            "range": "± 5544",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 729726,
            "range": "± 20144",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 571541,
            "range": "± 18345",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 579426,
            "range": "± 15790",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 275867,
            "range": "± 13211",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 566446,
            "range": "± 11320",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 575502,
            "range": "± 24483",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 565437,
            "range": "± 14886",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 369831,
            "range": "± 8499",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 316101,
            "range": "± 14595",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3934,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 16630932,
            "range": "± 25975",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 33018563,
            "range": "± 141710",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 65388357,
            "range": "± 149451",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 28682192,
            "range": "± 54887",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 29432399,
            "range": "± 69090",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 56241278,
            "range": "± 119834",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 58591779,
            "range": "± 159692",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 669562,
            "range": "± 22592",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 2530745,
            "range": "± 90902",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 5194355,
            "range": "± 281067",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1330876,
            "range": "± 16337",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 5347208,
            "range": "± 70559",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 11639420,
            "range": "± 423559",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 202830,
            "range": "± 6559",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 319902,
            "range": "± 10570",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 507261,
            "range": "± 9419",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 82365,
            "range": "± 2107",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 363752,
            "range": "± 23854",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 714862,
            "range": "± 16319",
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
          "id": "95d4c6e75240c85b899bf65cd4adb46ebf72fcfa",
          "message": "chore: bump version to 0.7.1",
          "timestamp": "2026-03-17T10:07:31Z",
          "tree_id": "4f7c7dd2806bcb41bd9e560ae34a0b318059478f",
          "url": "https://github.com/jburrow/fast_code_search/commit/95d4c6e75240c85b899bf65cd4adb46ebf72fcfa"
        },
        "date": 1773743207092,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 319931,
            "range": "± 13827",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 20532,
            "range": "± 242",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 405,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 568552,
            "range": "± 16586",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 20736,
            "range": "± 351",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 500,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 1056488,
            "range": "± 18377",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 21334,
            "range": "± 533",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 709,
            "range": "± 28",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 451965,
            "range": "± 25707",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 726149,
            "range": "± 37105",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 613871,
            "range": "± 21483",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 900029,
            "range": "± 4060",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 569658,
            "range": "± 17124",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 308497,
            "range": "± 2464",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 567991,
            "range": "± 3909",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 729921,
            "range": "± 4308",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 570866,
            "range": "± 20881",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 578260,
            "range": "± 17879",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 269992,
            "range": "± 9847",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 569646,
            "range": "± 10750",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 566658,
            "range": "± 13675",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 561905,
            "range": "± 15926",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 365719,
            "range": "± 11655",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 317674,
            "range": "± 14450",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3927,
            "range": "± 21",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 16637999,
            "range": "± 62220",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 32929692,
            "range": "± 78246",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 65326833,
            "range": "± 204824",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 28432922,
            "range": "± 85538",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 29272145,
            "range": "± 73022",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 56260796,
            "range": "± 168735",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 58408516,
            "range": "± 272166",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 660185,
            "range": "± 14108",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 2503917,
            "range": "± 36495",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 4870945,
            "range": "± 202341",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1317103,
            "range": "± 10491",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 5248878,
            "range": "± 56675",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 11261451,
            "range": "± 255244",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 208385,
            "range": "± 2678",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 326926,
            "range": "± 6370",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 510330,
            "range": "± 4830",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 82382,
            "range": "± 2944",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 363408,
            "range": "± 6438",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 708322,
            "range": "± 7107",
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
          "id": "9e32307aaddc9b1bf9bd68ab5aa180be9097e79c",
          "message": "[WIP] Fix linting issues in the codebase (#94)\n\n* Initial plan\n\n* fix: apply rustfmt formatting to engine.rs and path_filter.rs\n\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>\n\n---------\n\nCo-authored-by: copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>",
          "timestamp": "2026-03-17T11:03:41Z",
          "tree_id": "bf9c04fbe55c18b9852b01abe550ec39fd0c89dc",
          "url": "https://github.com/jburrow/fast_code_search/commit/9e32307aaddc9b1bf9bd68ab5aa180be9097e79c"
        },
        "date": 1773746068419,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 334929,
            "range": "± 54128",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 19583,
            "range": "± 700",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 404,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 580300,
            "range": "± 12292",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 20168,
            "range": "± 783",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 502,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 1074082,
            "range": "± 25294",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 20540,
            "range": "± 489",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 707,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 411137,
            "range": "± 14786",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 725198,
            "range": "± 22814",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 585842,
            "range": "± 20523",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 962548,
            "range": "± 29078",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 580951,
            "range": "± 21269",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 312798,
            "range": "± 9418",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 563640,
            "range": "± 22798",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 740478,
            "range": "± 17359",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 563711,
            "range": "± 27283",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 581581,
            "range": "± 16725",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 271894,
            "range": "± 21798",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 586417,
            "range": "± 19020",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 567071,
            "range": "± 14296",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 587250,
            "range": "± 38856",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 367624,
            "range": "± 18411",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 322178,
            "range": "± 16894",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3976,
            "range": "± 80",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 16607170,
            "range": "± 66956",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 32938223,
            "range": "± 81348",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 65557101,
            "range": "± 153647",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 28446296,
            "range": "± 47415",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 29507677,
            "range": "± 1380768",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 56453243,
            "range": "± 133979",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 59263468,
            "range": "± 241378",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 640631,
            "range": "± 20664",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 2505678,
            "range": "± 101010",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 4878043,
            "range": "± 133947",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1375851,
            "range": "± 50042",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 5324465,
            "range": "± 246954",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 10533802,
            "range": "± 364462",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 193870,
            "range": "± 7046",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 314148,
            "range": "± 35824",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 504387,
            "range": "± 12265",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 81547,
            "range": "± 2216",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 359962,
            "range": "± 10312",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 706152,
            "range": "± 5781",
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
          "id": "167f67109ccc2fec4ae224e7cbde3f757e339c75",
          "message": "feat: workspace-relative display paths (VSCode-style folder name in results) (#95)\n\n* Initial plan\n\n* feat: workspace-relative display paths (VSCode-style folder name in results)\n\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>\n\n---------\n\nCo-authored-by: copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>",
          "timestamp": "2026-03-18T07:04:08Z",
          "tree_id": "8984e03fb8e7e73e011b46a6c4e0e7563bf12d05",
          "url": "https://github.com/jburrow/fast_code_search/commit/167f67109ccc2fec4ae224e7cbde3f757e339c75"
        },
        "date": 1773818085023,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 302277,
            "range": "± 7760",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 23551,
            "range": "± 519",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 504,
            "range": "± 15",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 514585,
            "range": "± 45384",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 24056,
            "range": "± 578",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 657,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 938654,
            "range": "± 41801",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 24577,
            "range": "± 695",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 978,
            "range": "± 49",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 360408,
            "range": "± 23913",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 596575,
            "range": "± 22543",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 534931,
            "range": "± 20339",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 826900,
            "range": "± 5177",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 507618,
            "range": "± 19967",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 355842,
            "range": "± 5445",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 525031,
            "range": "± 5733",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 704986,
            "range": "± 4653",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 510446,
            "range": "± 17763",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 512803,
            "range": "± 26895",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 263266,
            "range": "± 16131",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 512981,
            "range": "± 27350",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 512905,
            "range": "± 47213",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 529681,
            "range": "± 47568",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 340710,
            "range": "± 8613",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 285449,
            "range": "± 15700",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3823,
            "range": "± 42",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18405766,
            "range": "± 20722",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36475778,
            "range": "± 119634",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 72497789,
            "range": "± 236692",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31708154,
            "range": "± 333686",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32868202,
            "range": "± 79534",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63427985,
            "range": "± 215753",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65783192,
            "range": "± 183237",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 902770,
            "range": "± 33689",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3410148,
            "range": "± 29686",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6495993,
            "range": "± 148581",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1974340,
            "range": "± 33332",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8118252,
            "range": "± 67305",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16676683,
            "range": "± 130166",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 198812,
            "range": "± 3544",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 316491,
            "range": "± 12687",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 474327,
            "range": "± 13314",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147210,
            "range": "± 1599",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 666612,
            "range": "± 6699",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1317905,
            "range": "± 48373",
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
          "id": "bdf9e5a9ea67431676eb2cfaca541c1b18c59a99",
          "message": "Add new design enhancement skills: arrange, audit, bolder, colorize, critique, and delight\n\n- Introduced 'arrange' skill for improving layout and visual hierarchy.\n- Added 'audit' skill for comprehensive interface quality assessments.\n- Created 'bolder' skill to amplify designs for greater visual impact.\n- Developed 'colorize' skill to strategically introduce color into monochromatic designs.\n- Implemented 'critique' skill for evaluating design effectiveness from a UX perspective.\n- Launched 'delight' skill to add moments of joy and personality to interfaces.\n- Updated skills-lock.json to include new skills with computed hashes.",
          "timestamp": "2026-03-18T07:15:43Z",
          "tree_id": "900d7c19092c4011cfbcfb12da3ad705c0a6e475",
          "url": "https://github.com/jburrow/fast_code_search/commit/bdf9e5a9ea67431676eb2cfaca541c1b18c59a99"
        },
        "date": 1773818793550,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 302910,
            "range": "± 49225",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 23933,
            "range": "± 542",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 489,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 515557,
            "range": "± 25336",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 23218,
            "range": "± 782",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 641,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 930861,
            "range": "± 34377",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 24680,
            "range": "± 233",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 965,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 374019,
            "range": "± 16832",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 605808,
            "range": "± 13663",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 512871,
            "range": "± 32728",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 829513,
            "range": "± 54673",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 508680,
            "range": "± 9566",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 348512,
            "range": "± 4537",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 554776,
            "range": "± 39974",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 706445,
            "range": "± 3654",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 510717,
            "range": "± 15818",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 519295,
            "range": "± 15602",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 258243,
            "range": "± 22043",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 509484,
            "range": "± 40412",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 508172,
            "range": "± 11792",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 505913,
            "range": "± 27430",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 336528,
            "range": "± 11580",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 286662,
            "range": "± 15493",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3838,
            "range": "± 37",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18370524,
            "range": "± 69229",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36367776,
            "range": "± 787902",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 72171893,
            "range": "± 253530",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31789100,
            "range": "± 64317",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32794186,
            "range": "± 92529",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63148845,
            "range": "± 206032",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65915362,
            "range": "± 175804",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 910060,
            "range": "± 19695",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3411002,
            "range": "± 32937",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6439860,
            "range": "± 54471",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1935795,
            "range": "± 14695",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7818146,
            "range": "± 92549",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 15695001,
            "range": "± 79571",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 200336,
            "range": "± 4491",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 317985,
            "range": "± 15942",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 474479,
            "range": "± 29602",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147440,
            "range": "± 3560",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 669606,
            "range": "± 5235",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1324995,
            "range": "± 25140",
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
          "id": "af3092eb5ee21562735a9e6a301d18f8db3a9baf",
          "message": "feat: enhance UI with wordmark styling and improve accessibility features",
          "timestamp": "2026-03-18T07:28:47Z",
          "tree_id": "61a44d714ac3abc8187c030ae57e0856a5110881",
          "url": "https://github.com/jburrow/fast_code_search/commit/af3092eb5ee21562735a9e6a301d18f8db3a9baf"
        },
        "date": 1773819604530,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 306192,
            "range": "± 55748",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22736,
            "range": "± 416",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 486,
            "range": "± 12",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 515954,
            "range": "± 39304",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 23250,
            "range": "± 227",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 639,
            "range": "± 59",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 941755,
            "range": "± 38462",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 23885,
            "range": "± 407",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 955,
            "range": "± 62",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 373568,
            "range": "± 23516",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 619113,
            "range": "± 31865",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 535324,
            "range": "± 22117",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 830085,
            "range": "± 6254",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 510868,
            "range": "± 13179",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 352027,
            "range": "± 3868",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 533862,
            "range": "± 31937",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 705959,
            "range": "± 52619",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 505570,
            "range": "± 19671",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 515270,
            "range": "± 16550",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 251620,
            "range": "± 15546",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 508151,
            "range": "± 14552",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 512710,
            "range": "± 19084",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 512452,
            "range": "± 18032",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 341456,
            "range": "± 10522",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 289311,
            "range": "± 21051",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3782,
            "range": "± 21",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18611414,
            "range": "± 105713",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36986160,
            "range": "± 127276",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 72710333,
            "range": "± 233291",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31850007,
            "range": "± 84931",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32988008,
            "range": "± 182243",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63179170,
            "range": "± 226680",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65691603,
            "range": "± 204195",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 931016,
            "range": "± 41340",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3427825,
            "range": "± 71061",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6643702,
            "range": "± 258818",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1961666,
            "range": "± 13104",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7903687,
            "range": "± 108895",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16125185,
            "range": "± 290126",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 201792,
            "range": "± 3031",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 322952,
            "range": "± 15731",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 474569,
            "range": "± 10084",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147888,
            "range": "± 1621",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 670671,
            "range": "± 29188",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1323445,
            "range": "± 16177",
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
          "id": "129642ba555631d2f263b65601b43ae3a0ec32f4",
          "message": "feat: update header link to use a help icon for documentation access",
          "timestamp": "2026-03-18T07:33:37Z",
          "tree_id": "5ceef8eeb1462f1b5a774a532783b09d9c64a983",
          "url": "https://github.com/jburrow/fast_code_search/commit/129642ba555631d2f263b65601b43ae3a0ec32f4"
        },
        "date": 1773820237785,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 306553,
            "range": "± 78832",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22696,
            "range": "± 340",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 488,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 512527,
            "range": "± 19666",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 23101,
            "range": "± 207",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 638,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 931459,
            "range": "± 39400",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 24036,
            "range": "± 357",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 954,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 367934,
            "range": "± 13981",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 593842,
            "range": "± 11926",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 524512,
            "range": "± 31663",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 825280,
            "range": "± 5165",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 512720,
            "range": "± 17581",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 351086,
            "range": "± 6958",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 536397,
            "range": "± 4025",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 722226,
            "range": "± 30769",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 520154,
            "range": "± 45755",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 519267,
            "range": "± 13961",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 258074,
            "range": "± 16818",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 512872,
            "range": "± 19694",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 509803,
            "range": "± 11273",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 507611,
            "range": "± 16477",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 332766,
            "range": "± 30208",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 279654,
            "range": "± 17248",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3936,
            "range": "± 61",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18608593,
            "range": "± 64431",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36996630,
            "range": "± 111873",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 73046634,
            "range": "± 198134",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31978565,
            "range": "± 160331",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32841643,
            "range": "± 192040",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63152275,
            "range": "± 118871",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65630875,
            "range": "± 82322",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 909441,
            "range": "± 23364",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3406010,
            "range": "± 44031",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6534658,
            "range": "± 189015",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1982174,
            "range": "± 15803",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8102133,
            "range": "± 88021",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16550284,
            "range": "± 217530",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 198525,
            "range": "± 3333",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 312789,
            "range": "± 13712",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 468060,
            "range": "± 7778",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147549,
            "range": "± 2341",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 669374,
            "range": "± 2776",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1320918,
            "range": "± 19718",
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
          "id": "f142999dd606643290d4ea1c454b8cdf8a1338e0",
          "message": "feat: enhance touch target sizes for better mobile accessibility",
          "timestamp": "2026-03-18T07:42:00Z",
          "tree_id": "08b49c8e83e6a20123e053e393d6f0b14ccd772a",
          "url": "https://github.com/jburrow/fast_code_search/commit/f142999dd606643290d4ea1c454b8cdf8a1338e0"
        },
        "date": 1773820894855,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 300226,
            "range": "± 10831",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22097,
            "range": "± 235",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 494,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 505470,
            "range": "± 12271",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 22685,
            "range": "± 331",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 653,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 912993,
            "range": "± 32015",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 23290,
            "range": "± 290",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 963,
            "range": "± 31",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 372106,
            "range": "± 27668",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 602048,
            "range": "± 26207",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 533075,
            "range": "± 21632",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 824302,
            "range": "± 13776",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 515865,
            "range": "± 30334",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 351313,
            "range": "± 5365",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 527086,
            "range": "± 3039",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 701745,
            "range": "± 6612",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 512814,
            "range": "± 18282",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 516871,
            "range": "± 22295",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 268820,
            "range": "± 23611",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 539751,
            "range": "± 29054",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 512715,
            "range": "± 25508",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 511839,
            "range": "± 35529",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 334016,
            "range": "± 10508",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 287075,
            "range": "± 21425",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3784,
            "range": "± 29",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18724222,
            "range": "± 52214",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36800136,
            "range": "± 355357",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 73309641,
            "range": "± 250015",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 32298991,
            "range": "± 143432",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 33101673,
            "range": "± 123831",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63879036,
            "range": "± 244306",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 66508433,
            "range": "± 203302",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 902066,
            "range": "± 23390",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3420361,
            "range": "± 59430",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 7002328,
            "range": "± 420981",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1950030,
            "range": "± 29070",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7948304,
            "range": "± 90309",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16772326,
            "range": "± 635447",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 199435,
            "range": "± 3029",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 318773,
            "range": "± 12956",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 477735,
            "range": "± 9258",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 148038,
            "range": "± 4068",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 671330,
            "range": "± 3939",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1330282,
            "range": "± 25863",
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
          "id": "c027c59c2b2628c853679c31487c688ccc81fc5a",
          "message": "colorize: Add strategic color to the web UI (#96)\n\n* Initial plan\n\n* colorize: Add strategic color to web UI static files\n\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>\n\n---------\n\nCo-authored-by: copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>",
          "timestamp": "2026-03-18T09:32:16Z",
          "tree_id": "d7d12ce9bb8a2666dc961f7a4f4ea0f05c46bf5b",
          "url": "https://github.com/jburrow/fast_code_search/commit/c027c59c2b2628c853679c31487c688ccc81fc5a"
        },
        "date": 1773826984988,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 339598,
            "range": "± 80321",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22561,
            "range": "± 432",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 459,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 514248,
            "range": "± 23300",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 22867,
            "range": "± 375",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 585,
            "range": "± 23",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 938964,
            "range": "± 30368",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 23693,
            "range": "± 417",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 844,
            "range": "± 32",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 372064,
            "range": "± 18572",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 598970,
            "range": "± 28207",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 521738,
            "range": "± 22484",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 830910,
            "range": "± 89396",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 500942,
            "range": "± 15842",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 354545,
            "range": "± 5923",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 534922,
            "range": "± 16080",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 707401,
            "range": "± 12357",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 508891,
            "range": "± 17768",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 512613,
            "range": "± 26620",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 250789,
            "range": "± 4167",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 507390,
            "range": "± 20864",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 509174,
            "range": "± 29621",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 526542,
            "range": "± 52337",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 338795,
            "range": "± 15862",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 287815,
            "range": "± 24813",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3779,
            "range": "± 19",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18652074,
            "range": "± 122104",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 37003167,
            "range": "± 235529",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 73590177,
            "range": "± 434583",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 32452081,
            "range": "± 136972",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 33267906,
            "range": "± 226171",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63499875,
            "range": "± 542016",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 66929411,
            "range": "± 286235",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 885866,
            "range": "± 25763",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3831412,
            "range": "± 169679",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 7618753,
            "range": "± 491445",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1961246,
            "range": "± 30711",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8176083,
            "range": "± 359725",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 17401991,
            "range": "± 564506",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 185433,
            "range": "± 10262",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 327678,
            "range": "± 14390",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 464860,
            "range": "± 9291",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147659,
            "range": "± 3189",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 669285,
            "range": "± 5774",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1321473,
            "range": "± 53259",
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
          "id": "ddeb22b198ea7c1d9fbd01a1fe5b91ca02a8c750",
          "message": "Fix benchmark CI failure: dirty Cargo.lock blocks gh-pages branch switch (#97)\n\n* Initial plan\n\n* Fix benchmark CI failure: reset local changes before gh-pages branch switch\n\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>\n\n---------\n\nCo-authored-by: copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>",
          "timestamp": "2026-03-18T18:03:05Z",
          "tree_id": "4aa1e1ff5cb30a35da8dfae2773fa7a83ebb462d",
          "url": "https://github.com/jburrow/fast_code_search/commit/ddeb22b198ea7c1d9fbd01a1fe5b91ca02a8c750"
        },
        "date": 1773857635196,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 308440,
            "range": "± 70096",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22528,
            "range": "± 224",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 489,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 513208,
            "range": "± 12650",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 22954,
            "range": "± 418",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 639,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 934014,
            "range": "± 26892",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 23990,
            "range": "± 259",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 955,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 366954,
            "range": "± 16496",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 596706,
            "range": "± 15833",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 516552,
            "range": "± 28600",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 828096,
            "range": "± 6179",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 508826,
            "range": "± 15474",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 355801,
            "range": "± 3757",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 522699,
            "range": "± 5695",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 704298,
            "range": "± 8404",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 508792,
            "range": "± 28494",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 517133,
            "range": "± 15351",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 261647,
            "range": "± 21546",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 512856,
            "range": "± 25008",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 512438,
            "range": "± 12067",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 501845,
            "range": "± 18886",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 339067,
            "range": "± 13039",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 284231,
            "range": "± 14131",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3787,
            "range": "± 15",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18701508,
            "range": "± 89025",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 37241164,
            "range": "± 169658",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 73883559,
            "range": "± 385657",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 32418147,
            "range": "± 106228",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 33258408,
            "range": "± 204502",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 64455088,
            "range": "± 167145",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 67168329,
            "range": "± 217031",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 891639,
            "range": "± 38732",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3392680,
            "range": "± 190181",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6520355,
            "range": "± 369217",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 2000785,
            "range": "± 13029",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8246538,
            "range": "± 133956",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16560912,
            "range": "± 302905",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 197884,
            "range": "± 3604",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 320849,
            "range": "± 12950",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 469449,
            "range": "± 12475",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147916,
            "range": "± 1875",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 672819,
            "range": "± 3142",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1331544,
            "range": "± 104083",
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
          "id": "fec20de7065ed9a35790780a591c5d4cf747a1f2",
          "message": "feat: update scoring factors and tooltip information in search results",
          "timestamp": "2026-03-20T07:31:25Z",
          "tree_id": "23bdba117260f4e72ba59dc234e24fc9740c9899",
          "url": "https://github.com/jburrow/fast_code_search/commit/fec20de7065ed9a35790780a591c5d4cf747a1f2"
        },
        "date": 1773992542310,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 302328,
            "range": "± 13416",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 23762,
            "range": "± 502",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 486,
            "range": "± 12",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 514285,
            "range": "± 44038",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 23965,
            "range": "± 333",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 646,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 931490,
            "range": "± 28043",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 25279,
            "range": "± 791",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 961,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 371298,
            "range": "± 18478",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 603101,
            "range": "± 19315",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 527718,
            "range": "± 15254",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 835951,
            "range": "± 7326",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 508107,
            "range": "± 19351",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 353351,
            "range": "± 5105",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 541548,
            "range": "± 26757",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 715678,
            "range": "± 43369",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 513517,
            "range": "± 30914",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 514360,
            "range": "± 9084",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 254823,
            "range": "± 20259",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 509289,
            "range": "± 27259",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 514129,
            "range": "± 22976",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 497423,
            "range": "± 21739",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 338802,
            "range": "± 13703",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 279621,
            "range": "± 18116",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3802,
            "range": "± 17",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18372523,
            "range": "± 90878",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36488484,
            "range": "± 137269",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 72630147,
            "range": "± 275796",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31975592,
            "range": "± 127877",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32843352,
            "range": "± 71075",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63035999,
            "range": "± 204345",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65720489,
            "range": "± 130074",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 888411,
            "range": "± 13737",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3409006,
            "range": "± 54287",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6515850,
            "range": "± 63759",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1940608,
            "range": "± 14995",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7872245,
            "range": "± 128409",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16195560,
            "range": "± 178336",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 201041,
            "range": "± 6925",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 316097,
            "range": "± 16800",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 471283,
            "range": "± 13784",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 148248,
            "range": "± 5751",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 672600,
            "range": "± 6402",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1324856,
            "range": "± 34912",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}