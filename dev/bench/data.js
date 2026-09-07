window.BENCHMARK_DATA = {
  "lastUpdate": 1788772244417,
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
          "id": "af9dcafb093011547cd2d7b38d1b5cf471a84521",
          "message": "chore: release v0.7.3",
          "timestamp": "2026-03-20T09:54:53Z",
          "tree_id": "6a417d709134c4eac73f9f36c572d78b822c6f1c",
          "url": "https://github.com/jburrow/fast_code_search/commit/af9dcafb093011547cd2d7b38d1b5cf471a84521"
        },
        "date": 1774001136534,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 303390,
            "range": "± 74731",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22280,
            "range": "± 249",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 482,
            "range": "± 21",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 510924,
            "range": "± 21434",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 22538,
            "range": "± 321",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 635,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 922671,
            "range": "± 21689",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 23513,
            "range": "± 384",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 951,
            "range": "± 53",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 371469,
            "range": "± 9370",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 636051,
            "range": "± 33354",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 532744,
            "range": "± 17211",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 828435,
            "range": "± 4244",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 507852,
            "range": "± 22535",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 346063,
            "range": "± 5176",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 528313,
            "range": "± 5142",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 702093,
            "range": "± 4214",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 510779,
            "range": "± 26765",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 513729,
            "range": "± 14692",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 256398,
            "range": "± 23109",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 511368,
            "range": "± 33335",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 504728,
            "range": "± 26401",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 506330,
            "range": "± 21175",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 332396,
            "range": "± 14166",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 281612,
            "range": "± 14028",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3801,
            "range": "± 23",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18406921,
            "range": "± 77273",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36440663,
            "range": "± 103595",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 72498856,
            "range": "± 469553",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31733981,
            "range": "± 67602",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32589539,
            "range": "± 291111",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63062287,
            "range": "± 160185",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65434355,
            "range": "± 278510",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 895212,
            "range": "± 22456",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3406447,
            "range": "± 153663",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6409837,
            "range": "± 68502",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1924788,
            "range": "± 28386",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7823351,
            "range": "± 32657",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 15699308,
            "range": "± 412748",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 197808,
            "range": "± 8931",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 318744,
            "range": "± 11337",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 473668,
            "range": "± 11793",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147503,
            "range": "± 1805",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 671210,
            "range": "± 5465",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1325491,
            "range": "± 41498",
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
          "id": "8eaf56cb2a09e19812366710d8a7a79d0acb7859",
          "message": "fix: adjust tooltip maximum width for better responsiveness",
          "timestamp": "2026-03-20T13:52:20Z",
          "tree_id": "0c766b67e082397ca455188ab81080f0bfa528c7",
          "url": "https://github.com/jburrow/fast_code_search/commit/8eaf56cb2a09e19812366710d8a7a79d0acb7859"
        },
        "date": 1774015457028,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 311283,
            "range": "± 80906",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 23558,
            "range": "± 438",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 486,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 516658,
            "range": "± 9528",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 24203,
            "range": "± 421",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 639,
            "range": "± 34",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 935821,
            "range": "± 19714",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 24635,
            "range": "± 439",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 956,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 372055,
            "range": "± 10437",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 604086,
            "range": "± 32459",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 535814,
            "range": "± 21527",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 833885,
            "range": "± 6581",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 528561,
            "range": "± 31596",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 349066,
            "range": "± 4685",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 524952,
            "range": "± 5587",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 702461,
            "range": "± 13355",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 512094,
            "range": "± 30084",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 518404,
            "range": "± 21325",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 257598,
            "range": "± 21459",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 510720,
            "range": "± 13498",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 514961,
            "range": "± 21417",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 511092,
            "range": "± 30513",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 339998,
            "range": "± 16634",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 288786,
            "range": "± 27082",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3806,
            "range": "± 44",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18308300,
            "range": "± 85237",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36201356,
            "range": "± 90969",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 71627782,
            "range": "± 182839",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31476816,
            "range": "± 70259",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32274878,
            "range": "± 161695",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 61842295,
            "range": "± 149453",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 64382387,
            "range": "± 138381",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 877132,
            "range": "± 14292",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3362794,
            "range": "± 19833",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6505427,
            "range": "± 429426",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1999524,
            "range": "± 11337",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8256596,
            "range": "± 127471",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 17867782,
            "range": "± 509932",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 200709,
            "range": "± 3465",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 310793,
            "range": "± 10248",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 458527,
            "range": "± 10206",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 149002,
            "range": "± 3675",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 673500,
            "range": "± 2920",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1329704,
            "range": "± 22991",
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
          "id": "3632f3e621e306b603ea66e9ede697fc6768c30d",
          "message": "feat: add script to run keyword and semantic search servers concurrently",
          "timestamp": "2026-03-23T17:52:27Z",
          "tree_id": "e55ff77ed388a422f7f9b3b7b5bf5aab4df4f91e",
          "url": "https://github.com/jburrow/fast_code_search/commit/3632f3e621e306b603ea66e9ede697fc6768c30d"
        },
        "date": 1774289044558,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 304343,
            "range": "± 4599",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22203,
            "range": "± 366",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 487,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 516834,
            "range": "± 19241",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 22520,
            "range": "± 238",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 640,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 917513,
            "range": "± 22694",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 23509,
            "range": "± 297",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 954,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 356496,
            "range": "± 21232",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 606284,
            "range": "± 12680",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 536289,
            "range": "± 20035",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 834861,
            "range": "± 6649",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 510816,
            "range": "± 38424",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 358652,
            "range": "± 6887",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 532341,
            "range": "± 37602",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 702870,
            "range": "± 5152",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 509323,
            "range": "± 28447",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 516482,
            "range": "± 13806",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 263934,
            "range": "± 21670",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 506690,
            "range": "± 13787",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 516938,
            "range": "± 26127",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 508235,
            "range": "± 17579",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 334031,
            "range": "± 8969",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 289849,
            "range": "± 12304",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3874,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18808494,
            "range": "± 110991",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36948648,
            "range": "± 208581",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 73538623,
            "range": "± 264491",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 32170283,
            "range": "± 257902",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 33170844,
            "range": "± 174896",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63661794,
            "range": "± 212316",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 66469992,
            "range": "± 287878",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 898012,
            "range": "± 46211",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3360642,
            "range": "± 111015",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6510046,
            "range": "± 390415",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1936115,
            "range": "± 21808",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7999897,
            "range": "± 132934",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16103134,
            "range": "± 387309",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 201240,
            "range": "± 4828",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 304427,
            "range": "± 7972",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 450757,
            "range": "± 10206",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147329,
            "range": "± 1315",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 669244,
            "range": "± 5139",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1329119,
            "range": "± 31402",
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
          "id": "5f94f878cc8c8e77813623badfefbe0558713221",
          "message": "Add configurable context lines, persistent settings, and search history (keyword + semantic) (#98)\n\n* Initial plan\n\n* Add configurable context lines, localStorage settings persistence, and search history\n\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>\nAgent-Logs-Url: https://github.com/jburrow/fast_code_search/sessions/21b33e60-4273-4772-8033-ea90ac1be5f9\n\n* Port search history and localStorage settings persistence to semantic search UI\n\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>\nAgent-Logs-Url: https://github.com/jburrow/fast_code_search/sessions/51fedee5-6c60-4627-91ac-e958745dc345\n\n---------\n\nCo-authored-by: copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>",
          "timestamp": "2026-03-23T18:31:58Z",
          "tree_id": "0e9a2dc2e8427c6fc4d3c96129fd96abd6548e49",
          "url": "https://github.com/jburrow/fast_code_search/commit/5f94f878cc8c8e77813623badfefbe0558713221"
        },
        "date": 1774291404780,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 327201,
            "range": "± 46742",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 20552,
            "range": "± 283",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 408,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 588171,
            "range": "± 21384",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 20361,
            "range": "± 372",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 510,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 1060741,
            "range": "± 17353",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 20201,
            "range": "± 744",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 714,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 441426,
            "range": "± 12176",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 734678,
            "range": "± 22671",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 626824,
            "range": "± 18149",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 935210,
            "range": "± 6508",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 576143,
            "range": "± 18001",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 303934,
            "range": "± 3012",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 574922,
            "range": "± 3299",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 740752,
            "range": "± 4729",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 579593,
            "range": "± 53831",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 602378,
            "range": "± 26237",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 286679,
            "range": "± 18156",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 578770,
            "range": "± 20549",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 576198,
            "range": "± 15727",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 577633,
            "range": "± 14150",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 369646,
            "range": "± 12573",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 314690,
            "range": "± 11269",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3998,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 16622941,
            "range": "± 53031",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 32903009,
            "range": "± 287052",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 65463109,
            "range": "± 174969",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 28687922,
            "range": "± 150102",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 29673518,
            "range": "± 76324",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 56803765,
            "range": "± 282916",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 58979057,
            "range": "± 213776",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 681764,
            "range": "± 30702",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 2568764,
            "range": "± 37363",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 5452665,
            "range": "± 246893",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1347562,
            "range": "± 11973",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 5502054,
            "range": "± 105333",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 12262215,
            "range": "± 339241",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 206739,
            "range": "± 3117",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 326924,
            "range": "± 8430",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 515132,
            "range": "± 8401",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 83427,
            "range": "± 1085",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 368439,
            "range": "± 12535",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 718527,
            "range": "± 7945",
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
          "id": "ffdb4fece33a26c48ef4a396e275f8e97df0d82e",
          "message": "feat: compact search bar slides into sticky nav on scroll (#100)\n\n* Initial plan\n\n* feat: sticky compact search bar slides into top nav on scroll\n\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>\nAgent-Logs-Url: https://github.com/jburrow/fast_code_search/sessions/d48b844b-619e-49db-9e4d-4f9f1660cec4\n\n---------\n\nCo-authored-by: copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>\nCo-authored-by: jburrow <1444266+jburrow@users.noreply.github.com>",
          "timestamp": "2026-03-23T21:43:24Z",
          "tree_id": "54ceaad7e9338fa68a29bd1f4a66e3649e906220",
          "url": "https://github.com/jburrow/fast_code_search/commit/ffdb4fece33a26c48ef4a396e275f8e97df0d82e"
        },
        "date": 1774302859550,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 316749,
            "range": "± 61527",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 24667,
            "range": "± 314",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 493,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 533015,
            "range": "± 19407",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 24387,
            "range": "± 523",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 683,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 976849,
            "range": "± 39369",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 24575,
            "range": "± 396",
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
            "value": 363541,
            "range": "± 12156",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 603040,
            "range": "± 16716",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 537799,
            "range": "± 42505",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 846906,
            "range": "± 12445",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 531489,
            "range": "± 27542",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 351657,
            "range": "± 17610",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 543706,
            "range": "± 13709",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 739452,
            "range": "± 23141",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 531388,
            "range": "± 14092",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 539233,
            "range": "± 16184",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 276918,
            "range": "± 27125",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 533115,
            "range": "± 24950",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 529134,
            "range": "± 15820",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 519596,
            "range": "± 10628",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 355413,
            "range": "± 9486",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 300142,
            "range": "± 20905",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 4021,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18683553,
            "range": "± 101852",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 37294370,
            "range": "± 112578",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 74144411,
            "range": "± 356144",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 32416056,
            "range": "± 465943",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 33546844,
            "range": "± 198476",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 64738260,
            "range": "± 253435",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 67162435,
            "range": "± 457538",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 909215,
            "range": "± 30865",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3468333,
            "range": "± 57849",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 7183204,
            "range": "± 273476",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1962802,
            "range": "± 16979",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8039516,
            "range": "± 171367",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 17194270,
            "range": "± 498569",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 202748,
            "range": "± 7521",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 323766,
            "range": "± 17793",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 481141,
            "range": "± 8259",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147683,
            "range": "± 3098",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 669753,
            "range": "± 3535",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1325604,
            "range": "± 29412",
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
          "id": "e64fdad0450b6ee574f21a26db01de508210f5de",
          "message": "feat: add dynamic language badge styling based on configured colors",
          "timestamp": "2026-03-24T08:35:27Z",
          "tree_id": "32100a092f906651d346461f15e7ca7e248857f5",
          "url": "https://github.com/jburrow/fast_code_search/commit/e64fdad0450b6ee574f21a26db01de508210f5de"
        },
        "date": 1774341977356,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 321445,
            "range": "± 59052",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 24517,
            "range": "± 608",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 496,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 527358,
            "range": "± 33030",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 24847,
            "range": "± 368",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 649,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 967378,
            "range": "± 22894",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 25356,
            "range": "± 363",
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
            "value": 372990,
            "range": "± 23765",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 602861,
            "range": "± 32195",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 527067,
            "range": "± 26830",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 835162,
            "range": "± 5352",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 530427,
            "range": "± 37067",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 356953,
            "range": "± 5703",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 548411,
            "range": "± 4850",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 727267,
            "range": "± 5332",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 528532,
            "range": "± 15900",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 538021,
            "range": "± 21653",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 277172,
            "range": "± 19383",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 527446,
            "range": "± 22089",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 528887,
            "range": "± 13576",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 524628,
            "range": "± 24709",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 357104,
            "range": "± 14692",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 295394,
            "range": "± 12643",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 4061,
            "range": "± 79",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18476805,
            "range": "± 58123",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36191847,
            "range": "± 191594",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 72216123,
            "range": "± 52081",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31749622,
            "range": "± 129405",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32818220,
            "range": "± 70263",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 62954360,
            "range": "± 267033",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65801390,
            "range": "± 302963",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 908350,
            "range": "± 24896",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3406222,
            "range": "± 86552",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6484772,
            "range": "± 106496",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 2005753,
            "range": "± 12711",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8149865,
            "range": "± 104125",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16508686,
            "range": "± 306383",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 205527,
            "range": "± 7484",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 328749,
            "range": "± 23430",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 483231,
            "range": "± 21168",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147428,
            "range": "± 2659",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 669000,
            "range": "± 3370",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1319606,
            "range": "± 13767",
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
          "id": "85c183c5c6c514454a58f2c7bf6ea2ddce753635",
          "message": "chore: release v0.7.5",
          "timestamp": "2026-03-24T09:28:01Z",
          "tree_id": "1db6b0fd92956b66fda195cadbf7971c27c28a54",
          "url": "https://github.com/jburrow/fast_code_search/commit/85c183c5c6c514454a58f2c7bf6ea2ddce753635"
        },
        "date": 1774345139037,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 307740,
            "range": "± 10296",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 23232,
            "range": "± 261",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 489,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 520085,
            "range": "± 17442",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 23773,
            "range": "± 463",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 643,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 938711,
            "range": "± 36008",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 24315,
            "range": "± 229",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 977,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 366378,
            "range": "± 9917",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 591703,
            "range": "± 13704",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 519284,
            "range": "± 15446",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 823051,
            "range": "± 6198",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 515023,
            "range": "± 14718",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 351468,
            "range": "± 5060",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 528714,
            "range": "± 5043",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 712444,
            "range": "± 5183",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 514621,
            "range": "± 13206",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 526279,
            "range": "± 17005",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 286166,
            "range": "± 28081",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 520323,
            "range": "± 33147",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 519316,
            "range": "± 11180",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 514850,
            "range": "± 12829",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 351144,
            "range": "± 50772",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 293116,
            "range": "± 12199",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 4100,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18237512,
            "range": "± 181755",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 36188259,
            "range": "± 132520",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 71897689,
            "range": "± 464783",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 31889124,
            "range": "± 207818",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 32724801,
            "range": "± 96986",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63083329,
            "range": "± 306073",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 65731697,
            "range": "± 187968",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 899796,
            "range": "± 23659",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3363682,
            "range": "± 47548",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6420624,
            "range": "± 270654",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1963660,
            "range": "± 13153",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8031100,
            "range": "± 52186",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16122988,
            "range": "± 118140",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 198508,
            "range": "± 5652",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 321873,
            "range": "± 13454",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 474105,
            "range": "± 6954",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 147446,
            "range": "± 3118",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 667671,
            "range": "± 3512",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1316225,
            "range": "± 18681",
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
          "id": "614959a2109412d9ea6cfa5ac2588c3fa9978064",
          "message": "feat: add immediate tooltip hiding function to improve UI responsiveness",
          "timestamp": "2026-03-27T07:10:11Z",
          "tree_id": "ec788b7cc6e4d324f6c7f14c08b09890fca9a225",
          "url": "https://github.com/jburrow/fast_code_search/commit/614959a2109412d9ea6cfa5ac2588c3fa9978064"
        },
        "date": 1774596279378,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 301016,
            "range": "± 4873",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 23165,
            "range": "± 472",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 492,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 510014,
            "range": "± 23951",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 23335,
            "range": "± 787",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 646,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 940900,
            "range": "± 43350",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 23844,
            "range": "± 744",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 960,
            "range": "± 27",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 362668,
            "range": "± 24631",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 601807,
            "range": "± 28482",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 537460,
            "range": "± 14723",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 831757,
            "range": "± 13050",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 518624,
            "range": "± 28235",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 357076,
            "range": "± 5499",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 525841,
            "range": "± 7255",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 708084,
            "range": "± 6310",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 514110,
            "range": "± 30638",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 518752,
            "range": "± 18551",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 255438,
            "range": "± 21606",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 513307,
            "range": "± 31645",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 505724,
            "range": "± 17165",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 500319,
            "range": "± 27824",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 333897,
            "range": "± 11690",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 286656,
            "range": "± 18630",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3779,
            "range": "± 32",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18865712,
            "range": "± 33593",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 37195429,
            "range": "± 131501",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 73877901,
            "range": "± 184125",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 32456183,
            "range": "± 122448",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 33357847,
            "range": "± 154560",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63908139,
            "range": "± 197468",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 66579340,
            "range": "± 434189",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 900986,
            "range": "± 19995",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3410137,
            "range": "± 97468",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6472875,
            "range": "± 133419",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1942573,
            "range": "± 14258",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7931970,
            "range": "± 31904",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 15933897,
            "range": "± 72888",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 203822,
            "range": "± 2305",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 320998,
            "range": "± 18152",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 484832,
            "range": "± 4178",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 148584,
            "range": "± 6403",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 671925,
            "range": "± 4114",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1333139,
            "range": "± 26612",
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
          "id": "4f5c3504593c9097f6d7ebcdea1f81417d5a4019",
          "message": "refactor: remove redundant syntax highlighting and tooltip styles, inherit from common.css",
          "timestamp": "2026-03-27T11:52:50Z",
          "tree_id": "ebe8d5ac9e576797a0f835ce1775ba4014af108c",
          "url": "https://github.com/jburrow/fast_code_search/commit/4f5c3504593c9097f6d7ebcdea1f81417d5a4019"
        },
        "date": 1774615413953,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 287127,
            "range": "± 8835",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22283,
            "range": "± 779",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 476,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 503847,
            "range": "± 32026",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 23440,
            "range": "± 851",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 600,
            "range": "± 18",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 935715,
            "range": "± 15777",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 23910,
            "range": "± 796",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 962,
            "range": "± 53",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 344623,
            "range": "± 8207",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 593965,
            "range": "± 11445",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 502577,
            "range": "± 5792",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 804638,
            "range": "± 3194",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 500285,
            "range": "± 13566",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 335842,
            "range": "± 2269",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 499942,
            "range": "± 3298",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 683423,
            "range": "± 5427",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 497988,
            "range": "± 15797",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 507242,
            "range": "± 26435",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 233645,
            "range": "± 4999",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 487111,
            "range": "± 15069",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 495655,
            "range": "± 12571",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 472777,
            "range": "± 8251",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 313165,
            "range": "± 9243",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 280339,
            "range": "± 8065",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3745,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 18694840,
            "range": "± 55646",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 37087610,
            "range": "± 42505",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 73597655,
            "range": "± 276218",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 32255866,
            "range": "± 50827",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 33082266,
            "range": "± 42874",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 63977528,
            "range": "± 273489",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 66294713,
            "range": "± 719660",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 867930,
            "range": "± 16912",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3245403,
            "range": "± 31290",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6094062,
            "range": "± 470018",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1799585,
            "range": "± 7919",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7299880,
            "range": "± 144176",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 14683293,
            "range": "± 49447",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 174890,
            "range": "± 2230",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 297474,
            "range": "± 10992",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 451340,
            "range": "± 1758",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 125593,
            "range": "± 2617",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 565881,
            "range": "± 17722",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1129152,
            "range": "± 37291",
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
          "id": "73b7477086f96772906698013ce000b190b74425",
          "message": "chore: bump version to 0.8.0 and update changelog",
          "timestamp": "2026-03-27T12:50:33Z",
          "tree_id": "2ee804198dbb9e4d429fa7f24cba2fc27fd7f666",
          "url": "https://github.com/jburrow/fast_code_search/commit/73b7477086f96772906698013ce000b190b74425"
        },
        "date": 1774616745148,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 285554,
            "range": "± 9039",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 22216,
            "range": "± 790",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 503,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 500479,
            "range": "± 15036",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 22273,
            "range": "± 746",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 664,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 944455,
            "range": "± 16210",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 23547,
            "range": "± 532",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 986,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 342607,
            "range": "± 12895",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 590128,
            "range": "± 19506",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 510830,
            "range": "± 8919",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 816767,
            "range": "± 32160",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 502717,
            "range": "± 37234",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 347706,
            "range": "± 8048",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 500795,
            "range": "± 5863",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 697969,
            "range": "± 10956",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 493018,
            "range": "± 40216",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 506186,
            "range": "± 13261",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 231198,
            "range": "± 4622",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 492839,
            "range": "± 19601",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 493109,
            "range": "± 8691",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 491033,
            "range": "± 12222",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 317039,
            "range": "± 15396",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 273497,
            "range": "± 26868",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3910,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 19285992,
            "range": "± 45244",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 38251001,
            "range": "± 132171",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 75997955,
            "range": "± 339081",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 33405813,
            "range": "± 192953",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 34317152,
            "range": "± 160022",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 65578336,
            "range": "± 240368",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 68352969,
            "range": "± 247478",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 873876,
            "range": "± 61963",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3666252,
            "range": "± 296882",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 7667834,
            "range": "± 310411",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1885372,
            "range": "± 36931",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8364739,
            "range": "± 240939",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 17467565,
            "range": "± 644539",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 173220,
            "range": "± 4221",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 302885,
            "range": "± 11213",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 449259,
            "range": "± 4402",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 126367,
            "range": "± 4973",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 570677,
            "range": "± 4891",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1131181,
            "range": "± 38728",
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
          "id": "1f4f1d60f5bf8a0b46d68ea2685320565437d5bb",
          "message": "feat: group search results by file for improved display and organization",
          "timestamp": "2026-03-27T14:54:05Z",
          "tree_id": "c802a2f4e0e2dd920bb54bccb89e5f54070576f4",
          "url": "https://github.com/jburrow/fast_code_search/commit/1f4f1d60f5bf8a0b46d68ea2685320565437d5bb"
        },
        "date": 1774632549098,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 315763,
            "range": "± 58016",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 18578,
            "range": "± 430",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 406,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 574223,
            "range": "± 12945",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 18812,
            "range": "± 320",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 503,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 1047992,
            "range": "± 12779",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 19680,
            "range": "± 367",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 700,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 414746,
            "range": "± 9028",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 681768,
            "range": "± 15270",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 589653,
            "range": "± 19487",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 875985,
            "range": "± 13158",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 569178,
            "range": "± 13315",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 290914,
            "range": "± 2057",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 544218,
            "range": "± 4312",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 715693,
            "range": "± 3814",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 564301,
            "range": "± 17035",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 578562,
            "range": "± 17405",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 253612,
            "range": "± 4332",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 569956,
            "range": "± 10468",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 562852,
            "range": "± 11734",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 559505,
            "range": "± 10710",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 336001,
            "range": "± 10513",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 292513,
            "range": "± 10584",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 4053,
            "range": "± 57",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 16461955,
            "range": "± 36908",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 32725673,
            "range": "± 175786",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 65196567,
            "range": "± 126389",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 28246368,
            "range": "± 121162",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 29073534,
            "range": "± 78271",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 55981609,
            "range": "± 106226",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 58805351,
            "range": "± 205955",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 655598,
            "range": "± 87246",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 2535160,
            "range": "± 17096",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 4727991,
            "range": "± 27877",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1307982,
            "range": "± 56068",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 5207434,
            "range": "± 40926",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 10997258,
            "range": "± 229463",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 183762,
            "range": "± 4064",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 315424,
            "range": "± 8990",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 491902,
            "range": "± 11753",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 80329,
            "range": "± 669",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 361796,
            "range": "± 4486",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 713079,
            "range": "± 4738",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "33a9732ab547a6467d97db613cd8958731eeed13",
          "message": "Fix/engine and UI improvements (#101)\n\n* fix(index): eliminate mmap UTF-8 UB, guard indexing panics, harden persistence\n\nPhase 1 + 2.3 of the engine improvement plan:\n- Remove from_utf8_unchecked + cached utf8_valid flag in file_store and\n  lazy_file_store; re-validate each read with safe conversions (fixes UB when\n  mmapped/fallback bytes change underneath a cached validity flag).\n- index_file now reads an owned buffer via the process/from_partial pipeline\n  instead of extracting through a live mmap (avoids truncation SIGBUS) and runs\n  the content-safety check before registering the file.\n- Wrap Phase-1 batch processing in catch_unwind; recover poisoned engine locks\n  via into_inner() instead of silently dropping batches.\n- Persistence: 8-byte magic header validated before decode; bincode decoded with\n  a file-size byte limit; atomic save via temp file + fsync + rename.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(index): correct reload id remapping + real incremental updates\n\nPhase 2 of the engine improvement plan:\n- Reload: remap trigram bitmap doc ids onto the compacted ids assigned during\n  load (built from actual add_file/register results), mirroring the symbol/dep\n  remap. Fixes silent result misattribution when any file is stale/removed.\n- Incremental updates: TrigramIndex::remove_document, LazyFileStore tombstone +\n  refresh, DependencyIndex::remove_file. update_file now strips stale data and\n  re-extracts under the same id; remove_file added. Watcher handles Modify(Name)\n  renames; main.rs wires Deleted/Renamed to engine removal.\n- Pipeline robustness: drain straggler files at shutdown, follow_links(false),\n  max_file_size plumbed through process()/engine/stale-files, saturating_sub on\n  elapsed time.\n- Tests: reload-remap round trip and incremental update/remove/rename.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(search): sound regex literals, glob exclusions, case-folding + empty-query guard\n\nPhase 3 of the engine improvement plan:\n- Regex acceleration: extract sound trigram constraints (intersection of\n  per-constraint unions). Alternations union their branch literals instead of\n  picking one 'best' literal, so 'hello|world' no longer drops files containing\n  only one branch; optional/min==0 subexprs add no constraint.\n- Exclusions: PathFilter::expand_pattern adds **/name/** for bare dir names;\n  watcher and stale-file filter now use the glob PathFilter instead of trimmed\n  substring matching (fixes .git excluding .github/.gitignore, Windows paths).\n- Case folding: verification is now Unicode-aware for non-ASCII needles\n  (matches the already-Unicode trigram layer), so über matches ÜBER end-to-end.\n- Empty/whitespace queries early-return instead of scanning the whole corpus.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(web): XSS hardening, grouping path, search race, lifecycle fixes\n\nPhase 4 of the engine/UI plan (web UI):\n- escapeHtml now escapes quotes; removed inline on* handlers (deps badge,\n  popover links, modal close) in favor of addEventListener + dataset, closing\n  an XSS vector via attacker-influenced file paths.\n- group-by-file uses the normalized path only as the map key and displays/fetches\n  the original file_path (fixes lowercased headers and 404s on case-sensitive FS).\n- performSearch aborts the in-flight request (AbortController) and Enter cancels\n  the pending debounce, eliminating stale-result races and double fetches.\n- offline banner re-checks health on WS connect/offline; shared-URL filters now\n  open the visible filter panel; always use the light highlight.js theme; ws->wss\n  under https and protocol-relative semantic probe; rebuilt tailwind.css with the\n  missing dropdown utilities; deleted dead/corrupt common.css and keyword.css.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(web): usability — filter discoverability, error bodies, history, keyboard nav\n\nPhase 5 of the engine/UI plan (web UI usability):\n- Always-visible FILTER toggle so filters can be set before the first search.\n- Surface server error bodies (invalid regex, index-updating) via readErrorBody.\n- submitSearch() records history only on explicit submit (no keystroke prefixes).\n- Progress panel auto-hides 4s after completion (no permanent 100% bar).\n- Keyboard nav: j/k or arrows move result selection, Enter opens, / focuses search.\n- Per-group copy-path button; results count shows truncation (has_more / page-full).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(api/web): JSON errors + Retry-After, ETag 304, lighter hover preview, docs\n\nPhase 6 of the engine/UI plan (API DX + perf):\n- ApiError type: all handlers return consistent JSON { error } bodies; 503s carry\n  Retry-After: 1. Conversion is via From, so handler call sites are unchanged.\n- Static assets use rust_embed's compile-time sha256 for the ETag (no per-request\n  hashing of the multi-MB font) and return 304 on If-None-Match.\n- SearchResponse gains has_more so the UI can show truncation honestly.\n- Hover preview uses the lightweight /api/context endpoint with a 200ms\n  hover-intent delay and a per-(path,line) cache instead of fetching whole files.\n- Docs page documents context param, /api/file|context|dependencies|diagnostics,\n  the JSON-error/Retry-After convention, and total_results/has_more semantics.\n- Cosmetics: drop stray 'relative' on sticky header, hide nav-search <640px,\n  plain mono ranking labels, remove dead searchTimeout.\n- Font subset (6.4) deferred: fonttools unavailable + shared ligature font needs\n  visual verification; documented as a follow-up.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: mark engine/UI improvement plan complete\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* chore: allowlist read-only cargo build/test and node --check\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: release v0.9.0\n\nEngine crash/correctness fixes + web UI/API improvements. See CHANGELOG.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-06-10T22:07:34+01:00",
          "tree_id": "ce2cf771d202134fb8372e001865f4f0937aeea0",
          "url": "https://github.com/jburrow/fast_code_search/commit/33a9732ab547a6467d97db613cd8958731eeed13"
        },
        "date": 1781126470475,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 301283,
            "range": "± 7737",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 24705,
            "range": "± 1407",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 491,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 524705,
            "range": "± 23170",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 24719,
            "range": "± 1531",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 592,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 970183,
            "range": "± 13110",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 24671,
            "range": "± 1396",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 954,
            "range": "± 51",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 364294,
            "range": "± 6540",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 617686,
            "range": "± 12487",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 525664,
            "range": "± 11173",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 832621,
            "range": "± 6717",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 525306,
            "range": "± 12635",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 346205,
            "range": "± 7619",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 534466,
            "range": "± 3608",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 727142,
            "range": "± 4914",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 518530,
            "range": "± 12572",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 528740,
            "range": "± 10507",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 254743,
            "range": "± 3384",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 525764,
            "range": "± 10350",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 520261,
            "range": "± 13487",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 519630,
            "range": "± 6712",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 336806,
            "range": "± 11828",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 292784,
            "range": "± 7892",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 4272,
            "range": "± 179",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 12056894,
            "range": "± 51827",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 23882316,
            "range": "± 101840",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 47293300,
            "range": "± 267532",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 20696339,
            "range": "± 46024",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 21579085,
            "range": "± 77755",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 41219239,
            "range": "± 208994",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 43551683,
            "range": "± 109534",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1301391,
            "range": "± 99344",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3923137,
            "range": "± 170349",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 7134334,
            "range": "± 130161",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1973538,
            "range": "± 27494",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7800892,
            "range": "± 18386",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 15603299,
            "range": "± 446978",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 186333,
            "range": "± 5915",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 302416,
            "range": "± 6960",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 451645,
            "range": "± 6894",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 125925,
            "range": "± 4350",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 564193,
            "range": "± 12036",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1128853,
            "range": "± 48240",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f25c92b06a1f17ab1da1942107baead5767eea03",
          "message": "v0.9.0 — engine crash/correctness fixes + web UI/API improvements (#102)\n\n* fix(index): eliminate mmap UTF-8 UB, guard indexing panics, harden persistence\n\nPhase 1 + 2.3 of the engine improvement plan:\n- Remove from_utf8_unchecked + cached utf8_valid flag in file_store and\n  lazy_file_store; re-validate each read with safe conversions (fixes UB when\n  mmapped/fallback bytes change underneath a cached validity flag).\n- index_file now reads an owned buffer via the process/from_partial pipeline\n  instead of extracting through a live mmap (avoids truncation SIGBUS) and runs\n  the content-safety check before registering the file.\n- Wrap Phase-1 batch processing in catch_unwind; recover poisoned engine locks\n  via into_inner() instead of silently dropping batches.\n- Persistence: 8-byte magic header validated before decode; bincode decoded with\n  a file-size byte limit; atomic save via temp file + fsync + rename.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(index): correct reload id remapping + real incremental updates\n\nPhase 2 of the engine improvement plan:\n- Reload: remap trigram bitmap doc ids onto the compacted ids assigned during\n  load (built from actual add_file/register results), mirroring the symbol/dep\n  remap. Fixes silent result misattribution when any file is stale/removed.\n- Incremental updates: TrigramIndex::remove_document, LazyFileStore tombstone +\n  refresh, DependencyIndex::remove_file. update_file now strips stale data and\n  re-extracts under the same id; remove_file added. Watcher handles Modify(Name)\n  renames; main.rs wires Deleted/Renamed to engine removal.\n- Pipeline robustness: drain straggler files at shutdown, follow_links(false),\n  max_file_size plumbed through process()/engine/stale-files, saturating_sub on\n  elapsed time.\n- Tests: reload-remap round trip and incremental update/remove/rename.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(search): sound regex literals, glob exclusions, case-folding + empty-query guard\n\nPhase 3 of the engine improvement plan:\n- Regex acceleration: extract sound trigram constraints (intersection of\n  per-constraint unions). Alternations union their branch literals instead of\n  picking one 'best' literal, so 'hello|world' no longer drops files containing\n  only one branch; optional/min==0 subexprs add no constraint.\n- Exclusions: PathFilter::expand_pattern adds **/name/** for bare dir names;\n  watcher and stale-file filter now use the glob PathFilter instead of trimmed\n  substring matching (fixes .git excluding .github/.gitignore, Windows paths).\n- Case folding: verification is now Unicode-aware for non-ASCII needles\n  (matches the already-Unicode trigram layer), so über matches ÜBER end-to-end.\n- Empty/whitespace queries early-return instead of scanning the whole corpus.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(web): XSS hardening, grouping path, search race, lifecycle fixes\n\nPhase 4 of the engine/UI plan (web UI):\n- escapeHtml now escapes quotes; removed inline on* handlers (deps badge,\n  popover links, modal close) in favor of addEventListener + dataset, closing\n  an XSS vector via attacker-influenced file paths.\n- group-by-file uses the normalized path only as the map key and displays/fetches\n  the original file_path (fixes lowercased headers and 404s on case-sensitive FS).\n- performSearch aborts the in-flight request (AbortController) and Enter cancels\n  the pending debounce, eliminating stale-result races and double fetches.\n- offline banner re-checks health on WS connect/offline; shared-URL filters now\n  open the visible filter panel; always use the light highlight.js theme; ws->wss\n  under https and protocol-relative semantic probe; rebuilt tailwind.css with the\n  missing dropdown utilities; deleted dead/corrupt common.css and keyword.css.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(web): usability — filter discoverability, error bodies, history, keyboard nav\n\nPhase 5 of the engine/UI plan (web UI usability):\n- Always-visible FILTER toggle so filters can be set before the first search.\n- Surface server error bodies (invalid regex, index-updating) via readErrorBody.\n- submitSearch() records history only on explicit submit (no keystroke prefixes).\n- Progress panel auto-hides 4s after completion (no permanent 100% bar).\n- Keyboard nav: j/k or arrows move result selection, Enter opens, / focuses search.\n- Per-group copy-path button; results count shows truncation (has_more / page-full).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(api/web): JSON errors + Retry-After, ETag 304, lighter hover preview, docs\n\nPhase 6 of the engine/UI plan (API DX + perf):\n- ApiError type: all handlers return consistent JSON { error } bodies; 503s carry\n  Retry-After: 1. Conversion is via From, so handler call sites are unchanged.\n- Static assets use rust_embed's compile-time sha256 for the ETag (no per-request\n  hashing of the multi-MB font) and return 304 on If-None-Match.\n- SearchResponse gains has_more so the UI can show truncation honestly.\n- Hover preview uses the lightweight /api/context endpoint with a 200ms\n  hover-intent delay and a per-(path,line) cache instead of fetching whole files.\n- Docs page documents context param, /api/file|context|dependencies|diagnostics,\n  the JSON-error/Retry-After convention, and total_results/has_more semantics.\n- Cosmetics: drop stray 'relative' on sticky header, hide nav-search <640px,\n  plain mono ranking labels, remove dead searchTimeout.\n- Font subset (6.4) deferred: fonttools unavailable + shared ligature font needs\n  visual verification; documented as a follow-up.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: mark engine/UI improvement plan complete\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* chore: allowlist read-only cargo build/test and node --check\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: release v0.9.0\n\nEngine crash/correctness fixes + web UI/API improvements. See CHANGELOG.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: polish README — hero, badges, quickstart, collapsibles, v0.9.0 API\n\n- Centered hero with tagline, CI/benchmark/version/license/Rust badges, quick-nav.\n- Top-of-file Quick Start (60s) and Highlights sections.\n- Emoji section headers and a table of contents via nav links.\n- Collapsible <details> for the deep tool comparison and the glossary.\n- Refreshed REST API table (file/context/diagnostics endpoints, rank/context\n  params, has_more, JSON errors) and corrected exclude-pattern docs to glob\n  semantics (matching the v0.9.0 behavior change).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-06-11T06:43:01+01:00",
          "tree_id": "943367584eb32acfbd26e8975f23174579a71275",
          "url": "https://github.com/jburrow/fast_code_search/commit/f25c92b06a1f17ab1da1942107baead5767eea03"
        },
        "date": 1781157239200,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 291221,
            "range": "± 28478",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 20464,
            "range": "± 731",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 472,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 513218,
            "range": "± 9634",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 21141,
            "range": "± 624",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 589,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 925914,
            "range": "± 13056",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 21046,
            "range": "± 1020",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 850,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 341397,
            "range": "± 7163",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 599841,
            "range": "± 25915",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 511029,
            "range": "± 10432",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 822539,
            "range": "± 9001",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 507798,
            "range": "± 8082",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 328579,
            "range": "± 5373",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 509136,
            "range": "± 6038",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 715952,
            "range": "± 7053",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 509764,
            "range": "± 10169",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 529391,
            "range": "± 11868",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 246016,
            "range": "± 4477",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 530004,
            "range": "± 16057",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 513785,
            "range": "± 13679",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 515261,
            "range": "± 12050",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 329579,
            "range": "± 7504",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 287918,
            "range": "± 6308",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 4222,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 11346403,
            "range": "± 124968",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 22217002,
            "range": "± 124965",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 44003588,
            "range": "± 188835",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 19453778,
            "range": "± 85502",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 20541207,
            "range": "± 47899",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 38780025,
            "range": "± 155398",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 41432922,
            "range": "± 124484",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1116968,
            "range": "± 62290",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3518024,
            "range": "± 54474",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6390978,
            "range": "± 124123",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 2037504,
            "range": "± 58944",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8211895,
            "range": "± 60491",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16488559,
            "range": "± 312170",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 174897,
            "range": "± 5132",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 294303,
            "range": "± 7168",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 450036,
            "range": "± 3760",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 133076,
            "range": "± 1590",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 600773,
            "range": "± 15272",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1204872,
            "range": "± 26022",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "27d1c02875927e42092dbcf01bebb6b30eb4d8ed",
          "message": "docs: add web UI screenshot to README hero (#103)\n\n* fix(index): eliminate mmap UTF-8 UB, guard indexing panics, harden persistence\n\nPhase 1 + 2.3 of the engine improvement plan:\n- Remove from_utf8_unchecked + cached utf8_valid flag in file_store and\n  lazy_file_store; re-validate each read with safe conversions (fixes UB when\n  mmapped/fallback bytes change underneath a cached validity flag).\n- index_file now reads an owned buffer via the process/from_partial pipeline\n  instead of extracting through a live mmap (avoids truncation SIGBUS) and runs\n  the content-safety check before registering the file.\n- Wrap Phase-1 batch processing in catch_unwind; recover poisoned engine locks\n  via into_inner() instead of silently dropping batches.\n- Persistence: 8-byte magic header validated before decode; bincode decoded with\n  a file-size byte limit; atomic save via temp file + fsync + rename.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(index): correct reload id remapping + real incremental updates\n\nPhase 2 of the engine improvement plan:\n- Reload: remap trigram bitmap doc ids onto the compacted ids assigned during\n  load (built from actual add_file/register results), mirroring the symbol/dep\n  remap. Fixes silent result misattribution when any file is stale/removed.\n- Incremental updates: TrigramIndex::remove_document, LazyFileStore tombstone +\n  refresh, DependencyIndex::remove_file. update_file now strips stale data and\n  re-extracts under the same id; remove_file added. Watcher handles Modify(Name)\n  renames; main.rs wires Deleted/Renamed to engine removal.\n- Pipeline robustness: drain straggler files at shutdown, follow_links(false),\n  max_file_size plumbed through process()/engine/stale-files, saturating_sub on\n  elapsed time.\n- Tests: reload-remap round trip and incremental update/remove/rename.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(search): sound regex literals, glob exclusions, case-folding + empty-query guard\n\nPhase 3 of the engine improvement plan:\n- Regex acceleration: extract sound trigram constraints (intersection of\n  per-constraint unions). Alternations union their branch literals instead of\n  picking one 'best' literal, so 'hello|world' no longer drops files containing\n  only one branch; optional/min==0 subexprs add no constraint.\n- Exclusions: PathFilter::expand_pattern adds **/name/** for bare dir names;\n  watcher and stale-file filter now use the glob PathFilter instead of trimmed\n  substring matching (fixes .git excluding .github/.gitignore, Windows paths).\n- Case folding: verification is now Unicode-aware for non-ASCII needles\n  (matches the already-Unicode trigram layer), so über matches ÜBER end-to-end.\n- Empty/whitespace queries early-return instead of scanning the whole corpus.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(web): XSS hardening, grouping path, search race, lifecycle fixes\n\nPhase 4 of the engine/UI plan (web UI):\n- escapeHtml now escapes quotes; removed inline on* handlers (deps badge,\n  popover links, modal close) in favor of addEventListener + dataset, closing\n  an XSS vector via attacker-influenced file paths.\n- group-by-file uses the normalized path only as the map key and displays/fetches\n  the original file_path (fixes lowercased headers and 404s on case-sensitive FS).\n- performSearch aborts the in-flight request (AbortController) and Enter cancels\n  the pending debounce, eliminating stale-result races and double fetches.\n- offline banner re-checks health on WS connect/offline; shared-URL filters now\n  open the visible filter panel; always use the light highlight.js theme; ws->wss\n  under https and protocol-relative semantic probe; rebuilt tailwind.css with the\n  missing dropdown utilities; deleted dead/corrupt common.css and keyword.css.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(web): usability — filter discoverability, error bodies, history, keyboard nav\n\nPhase 5 of the engine/UI plan (web UI usability):\n- Always-visible FILTER toggle so filters can be set before the first search.\n- Surface server error bodies (invalid regex, index-updating) via readErrorBody.\n- submitSearch() records history only on explicit submit (no keystroke prefixes).\n- Progress panel auto-hides 4s after completion (no permanent 100% bar).\n- Keyboard nav: j/k or arrows move result selection, Enter opens, / focuses search.\n- Per-group copy-path button; results count shows truncation (has_more / page-full).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(api/web): JSON errors + Retry-After, ETag 304, lighter hover preview, docs\n\nPhase 6 of the engine/UI plan (API DX + perf):\n- ApiError type: all handlers return consistent JSON { error } bodies; 503s carry\n  Retry-After: 1. Conversion is via From, so handler call sites are unchanged.\n- Static assets use rust_embed's compile-time sha256 for the ETag (no per-request\n  hashing of the multi-MB font) and return 304 on If-None-Match.\n- SearchResponse gains has_more so the UI can show truncation honestly.\n- Hover preview uses the lightweight /api/context endpoint with a 200ms\n  hover-intent delay and a per-(path,line) cache instead of fetching whole files.\n- Docs page documents context param, /api/file|context|dependencies|diagnostics,\n  the JSON-error/Retry-After convention, and total_results/has_more semantics.\n- Cosmetics: drop stray 'relative' on sticky header, hide nav-search <640px,\n  plain mono ranking labels, remove dead searchTimeout.\n- Font subset (6.4) deferred: fonttools unavailable + shared ligature font needs\n  visual verification; documented as a follow-up.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: mark engine/UI improvement plan complete\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* chore: allowlist read-only cargo build/test and node --check\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: release v0.9.0\n\nEngine crash/correctness fixes + web UI/API improvements. See CHANGELOG.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: polish README — hero, badges, quickstart, collapsibles, v0.9.0 API\n\n- Centered hero with tagline, CI/benchmark/version/license/Rust badges, quick-nav.\n- Top-of-file Quick Start (60s) and Highlights sections.\n- Emoji section headers and a table of contents via nav links.\n- Collapsible <details> for the deep tool comparison and the glossary.\n- Refreshed REST API table (file/context/diagnostics endpoints, rank/context\n  params, has_more, JSON errors) and corrected exclude-pattern docs to glob\n  semantics (matching the v0.9.0 behavior change).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: add web UI screenshot to README hero\n\nHeadless-Chrome capture (2x scale) of the live UI searching the project's own\nsource — shows grouped results, symbol badges, highlighting, and latency.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>\nCo-authored-by: copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>",
          "timestamp": "2026-06-11T07:38:45+01:00",
          "tree_id": "5d2e2cc9eb6a4f0f82387760c4c384c66a793b44",
          "url": "https://github.com/jburrow/fast_code_search/commit/27d1c02875927e42092dbcf01bebb6b30eb4d8ed"
        },
        "date": 1781160583099,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 291700,
            "range": "± 7303",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 20241,
            "range": "± 682",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 472,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 510467,
            "range": "± 11416",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 21323,
            "range": "± 597",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 593,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 951241,
            "range": "± 14728",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 21586,
            "range": "± 821",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 857,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 340517,
            "range": "± 6879",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 597893,
            "range": "± 13475",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 507377,
            "range": "± 8552",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 827253,
            "range": "± 5935",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 504305,
            "range": "± 11721",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 332438,
            "range": "± 5269",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 537296,
            "range": "± 44568",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 713692,
            "range": "± 8274",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 507721,
            "range": "± 13550",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 524880,
            "range": "± 12886",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 243606,
            "range": "± 6945",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 511811,
            "range": "± 16450",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 506264,
            "range": "± 8061",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 507126,
            "range": "± 13427",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 326043,
            "range": "± 9281",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 286806,
            "range": "± 8383",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 4293,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 11305386,
            "range": "± 52949",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 22276450,
            "range": "± 88437",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 44211669,
            "range": "± 366096",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 19631532,
            "range": "± 45891",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 20562573,
            "range": "± 91292",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 38900839,
            "range": "± 139172",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 41391918,
            "range": "± 290695",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1127135,
            "range": "± 87545",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3600084,
            "range": "± 82455",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6432782,
            "range": "± 301183",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 2031107,
            "range": "± 13285",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8166980,
            "range": "± 48447",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16453063,
            "range": "± 240334",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 175832,
            "range": "± 4663",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 299802,
            "range": "± 8469",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 452367,
            "range": "± 6982",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 135448,
            "range": "± 2828",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 600475,
            "range": "± 13209",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1196789,
            "range": "± 28344",
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
          "id": "b16c8fc5aaccaaaf4ba10fd494046c1b5ba3101b",
          "message": "docs: rewrite README in a restrained technical voice\n\nFull rewrite replacing the emoji-heavy layout: minimal hero (title, one-line\ndescription, three badges, screenshot), terse specific prose throughout, one\nconsolidated feature list instead of three overlapping ones, compact\narchitecture table, and a single ranking-signals table. Glossary moved to\ndocs/GLOSSARY.md. All link targets verified.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-06-11T07:44:14+01:00",
          "tree_id": "72e942e7ed110af61991fd92cc7090c3d61a9e77",
          "url": "https://github.com/jburrow/fast_code_search/commit/b16c8fc5aaccaaaf4ba10fd494046c1b5ba3101b"
        },
        "date": 1781161228540,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 290779,
            "range": "± 7477",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 20731,
            "range": "± 819",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 474,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 522833,
            "range": "± 18687",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 20048,
            "range": "± 704",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 595,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 954161,
            "range": "± 23596",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 20897,
            "range": "± 784",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 861,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 335030,
            "range": "± 8179",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 601637,
            "range": "± 11325",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 495965,
            "range": "± 12701",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 818757,
            "range": "± 9935",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 505328,
            "range": "± 11486",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 333321,
            "range": "± 12088",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 519881,
            "range": "± 9857",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 719800,
            "range": "± 5404",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 509561,
            "range": "± 11308",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 522502,
            "range": "± 15055",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 241941,
            "range": "± 4987",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 509282,
            "range": "± 58246",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 508169,
            "range": "± 8327",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 509202,
            "range": "± 12694",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 328493,
            "range": "± 7762",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 280374,
            "range": "± 8560",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 4215,
            "range": "± 107",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 11440135,
            "range": "± 87012",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 22461488,
            "range": "± 108322",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 44247515,
            "range": "± 73472",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 19556569,
            "range": "± 113363",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 20644784,
            "range": "± 60069",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 38771463,
            "range": "± 217014",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 41385166,
            "range": "± 262103",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1141196,
            "range": "± 48060",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3521877,
            "range": "± 78160",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6536938,
            "range": "± 293949",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 2008342,
            "range": "± 14672",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 8103001,
            "range": "± 62127",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16414020,
            "range": "± 278308",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 178882,
            "range": "± 6108",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 299230,
            "range": "± 6873",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 461268,
            "range": "± 5884",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 131253,
            "range": "± 2641",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 606981,
            "range": "± 9805",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1198859,
            "range": "± 19173",
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
          "id": "3ed71ac86f305da2d8643e8960af9ad3c6cb73c5",
          "message": "docs: cite benchmark provenance explicitly\n\nBenchmark table now states its exact source: CI run 27326492528 (linked),\ncommit f25c92b, runner, Criterion invocation and rounding, corpus definition\n(50-line synthetic Rust files from benches/search_benchmark.rs), and that\nsearches measure the query path against a pre-built index. The ripgrep figure\nis attributed to Andrew Gallant's 2016 benchmark post with its context.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-06-11T07:55:35+01:00",
          "tree_id": "7cfec3172007286ddab59ead7266309847287ab6",
          "url": "https://github.com/jburrow/fast_code_search/commit/3ed71ac86f305da2d8643e8960af9ad3c6cb73c5"
        },
        "date": 1781161879791,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 299972,
            "range": "± 5607",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 23194,
            "range": "± 1662",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 462,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 523367,
            "range": "± 22055",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 24117,
            "range": "± 1474",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 595,
            "range": "± 27",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 974303,
            "range": "± 17778",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 25799,
            "range": "± 1784",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 868,
            "range": "± 56",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 352185,
            "range": "± 10535",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 587710,
            "range": "± 13300",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 519857,
            "range": "± 15962",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 821320,
            "range": "± 11617",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 511031,
            "range": "± 6270",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 336118,
            "range": "± 5403",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 525269,
            "range": "± 15254",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 709531,
            "range": "± 4985",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 511506,
            "range": "± 11915",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 533830,
            "range": "± 13255",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 254450,
            "range": "± 4024",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 527838,
            "range": "± 16436",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 511054,
            "range": "± 30707",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 506218,
            "range": "± 7261",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 330538,
            "range": "± 5156",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 290068,
            "range": "± 6764",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 4202,
            "range": "± 17",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 11956790,
            "range": "± 84321",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 23571330,
            "range": "± 192546",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 47060209,
            "range": "± 131407",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 20807173,
            "range": "± 84361",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 21881260,
            "range": "± 116644",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 41270187,
            "range": "± 204035",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 44294043,
            "range": "± 625013",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1349540,
            "range": "± 117696",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 4028192,
            "range": "± 141798",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 7786582,
            "range": "± 410413",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1952693,
            "range": "± 48678",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 7926853,
            "range": "± 145155",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 16392553,
            "range": "± 449325",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 184847,
            "range": "± 4750",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 302881,
            "range": "± 7598",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 461756,
            "range": "± 5638",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 122212,
            "range": "± 1874",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 560654,
            "range": "± 10128",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 1116880,
            "range": "± 35740",
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
          "id": "241d01a3c7f4150323dc73259a666bf36bfa1859",
          "message": "docs: link benchmark trend dashboard on GitHub Pages\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-06-11T17:43:32+01:00",
          "tree_id": "72c1f5a9388d31d5eaa2c5789c7f3e1946a7c0f1",
          "url": "https://github.com/jburrow/fast_code_search/commit/241d01a3c7f4150323dc73259a666bf36bfa1859"
        },
        "date": 1781196815856,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 227411,
            "range": "± 6635",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 16407,
            "range": "± 455",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 367,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 400706,
            "range": "± 9923",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 16555,
            "range": "± 526",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 467,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 742535,
            "range": "± 16565",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 16960,
            "range": "± 474",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 664,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 274186,
            "range": "± 9496",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 481626,
            "range": "± 23810",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 412053,
            "range": "± 13805",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 658000,
            "range": "± 11043",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 414049,
            "range": "± 14206",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 265025,
            "range": "± 6443",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 411389,
            "range": "± 7350",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 579079,
            "range": "± 8555",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 419258,
            "range": "± 11976",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 424788,
            "range": "± 14832",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 196173,
            "range": "± 4496",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 394305,
            "range": "± 16103",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 419243,
            "range": "± 10751",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 402191,
            "range": "± 15479",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 256411,
            "range": "± 8690",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 226216,
            "range": "± 8115",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 3355,
            "range": "± 76",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 9052526,
            "range": "± 123490",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 17385716,
            "range": "± 173909",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 34063791,
            "range": "± 291067",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 15278682,
            "range": "± 112479",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 16256553,
            "range": "± 136985",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 30296959,
            "range": "± 272493",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 32748093,
            "range": "± 209559",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1083533,
            "range": "± 3490821",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3524329,
            "range": "± 8053162",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 7435108,
            "range": "± 13857838",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1563370,
            "range": "± 19917",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 6316401,
            "range": "± 125485",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 12988374,
            "range": "± 241407",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 142438,
            "range": "± 3537",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 237265,
            "range": "± 5854",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 359728,
            "range": "± 5723",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 102349,
            "range": "± 1327",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 456635,
            "range": "± 14710",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 921290,
            "range": "± 23347",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1b99b6a609fcb9312846d8e9c6b9b937dc754659",
          "message": "Keyword roadmap (#104)\n\n* chore: add .gitattributes to normalize line endings to LF\n\nThe working tree was a pure CRLF flip of 172 files with no .gitattributes.\nText files are now LF in the repo and on checkout; Windows scripts keep CRLF;\nbinary assets are marked so they are never converted.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* chore: repair .gitignore and untrack onnxruntime and test_corpus\n\n.gitignore had a UTF-16 fragment spliced into the onnxruntime/ line, so 23\nfiles including a 12 MB DLL were tracked, and *.zip was never ignored.\ntest_corpus/ held three gitlinks with no .gitmodules. Both are now untracked\nand ignored.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* chore: format and fix clippy/rustdoc warnings across all targets\n\nApplies cargo fmt (13 hunks) and clears the 8 clippy warnings and 1 rustdoc\nerror exposed by linting with --all-targets --all-features: is_none_or,\nsort_by_key, redundant closure, byte-string literals, struct-init in a test,\na stale doc block above regex_candidate_docs, and an unescaped generic in a\ndoc comment.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* build: set MSRV 1.89, pin toolchain 1.98.1, use rustls for reqwest\n\nThe docs claimed Rust 1.70+; the code uses File::lock_shared (1.89) and\nis_multiple_of (1.87) and the dependency tree needs 1.85, so rust-version is\nnow 1.89 (verified with cargo +1.89 check --all-targets). rust-toolchain.toml\npins 1.98.1 so rustfmt output is stable between CI and developers.\n\nreqwest (dev-dependency and the optional ml-models dependency) now uses\nrustls-tls so cargo test and --all-features builds no longer need system\nOpenSSL headers. The unused tokio-test dev-dependency is dropped and the\ncrate exclude list covers non-crate directories.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* ci: pin toolchain, widen lint, add MSRV job, gate release on tests\n\n- clippy now runs --all-targets --all-features -D warnings; cargo doc with\n  -D warnings\n- new MSRV job (cargo +1.89 check --all-targets)\n- release build job needs a test job (fmt + clippy + tests) on the tag\n- VS Code extension publishes on ext-v* tags instead of every v* tag\n- Swatinem/rust-cache replaces raw actions/cache on target/\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: add keyword engine review and roadmap (Phase 0 complete)\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(index): remap doc ids on save so removals don't corrupt the persisted index\n\nsave_index compacts the file table over tombstoned ids but wrote trigram\nbitmaps, symbols and dependency edges keyed by live id. After any\nremove_file (the watcher delete path), every file after the tombstone was\nmisattributed or lost on reload. Symbols, edges and bitmaps are now remapped\nfrom live id to file-table position at save time; when nothing was removed\nthe bitmaps are borrowed unchanged.\n\nAdds test_save_after_remove_keeps_ids_consistent, which failed before this\nchange with two of three surviving files returning no results.\n\nRoadmap 1.1 (P0).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(search): stop the first line of every file getting the definition boost\n\nThe synthetic FileName symbol lives at line 0 with is_definition = true and\nwas included in symbol_def_lines, so any match on line 1 of any file scored\n3x as if it were a symbol definition (shebangs, license headers, use lines\noutranked real definitions). FileName is now excluded in both the scored and\nregex per-document paths.\n\nAdds test_first_line_does_not_get_definition_boost (fails without the fix).\n\nRoadmap 1.2.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(index): keep long-line / deeply nested files text-searchable\n\ncontent_safety_check gated the whole file: a single line over 100 KB or\nbracket nesting over 500 dropped the file from the trigram index, even with\nsymbols disabled, so large JSON fixtures and generated code were\nunsearchable. The check is now split: binary-looking content is still\nskipped entirely, but the structural checks only set tree_sitter_safe on the\nPartialIndexedFile, and from_partial skips symbol/import extraction for\nthose files while their trigrams are indexed normally.\n\nAdds test_long_line_file_is_searchable_without_symbols.\n\nRoadmap 1.3.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(index): persist the mtime/size of the content that was indexed\n\nsave_index re-stat'ed every file at save time, so a file edited between\nindexing and a (possibly minutes-later) checkpoint was saved with its new\nmtime/size next to its old trigrams and never detected as stale on reload.\nPartialIndexedFile now captures (mtime, size) from the metadata read\nimmediately before the content; the engine keeps it per file id (seeded from\npersisted metadata on reload) and save_index writes that, only falling back\nto a stat for ids with no record. This also removes one stat per file from\nthe save path.\n\nAdds test_edit_between_index_and_save_is_detected_as_stale.\n\nRoadmap 1.4.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(symbols): TSX grammar, JS/TS method and arrow coverage, name-node positions\n\n- .tsx files were parsed with the TypeScript grammar, which rejects JSX, so\n  every React component body became ERROR nodes; they now use LANGUAGE_TSX.\n- JS/TS class members (method_definition: methods, getters, setters,\n  constructors), arrow-function and function-expression consts\n  (const Foo = () => ...), generator functions, abstract classes and\n  namespaces were not captured at all.\n- Symbol line/column came from the whole declaration node, so Java\n  @Override, TS decorators, Rust attributes and multi-line C signatures\n  reported the annotation's line and the definition boost landed there.\n  All arms now take the position of the name node.\n\nAdds test_tsx_react_component_extraction, test_javascript_methods_and_arrows\nand test_symbol_line_is_name_line (the first line-number assertions in the\nextractor tests).\n\nRoadmap 1.5.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(server): graceful shutdown with final index save\n\nSIGINT/SIGTERM previously killed the process outright: no index save, no\ntelemetry flush, and watcher-applied edits were lost unless\nsave_after_updates happened to fire. Now:\n\n- a shutdown task listens for Ctrl+C and (on Unix) SIGTERM and flips a\n  shared AtomicBool plus a watch channel\n- tonic uses serve_with_shutdown and axum with_graceful_shutdown\n- the background indexer's discovery and batch loops stop when the flag is\n  set and fall through to the normal finalize + save, so a partial build is\n  persisted as a checkpoint\n- the watcher loop exits on the flag and saves if it applied any updates\n  since the last periodic save (save_after_watcher_shutdown)\n- main joins the web task and both threads before flushing telemetry\n- the web listener is bound in main so a port conflict is fatal instead of\n  leaving a half-alive server with no REST API\n\nVerified manually: start with watch=true, append a function to a file,\nSIGTERM -> log shows the watcher save; restart loads 2 files from cache and\nthe new function is searchable with 0 stale files.\n\nRoadmap 1.6 (and the web-bind part of the P1 serving findings).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs(roadmap): tick 1.3-1.6\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(server): make the watcher write path panic-safe and recover from poisoned locks\n\nA panic while the watcher held the engine write lock poisoned it, after\nwhich every REST/gRPC search returned 500 until restart while the indexer\n(which already recovered from poison) kept going. Now:\n\n- watcher updates run through with_engine_write, which recovers a poisoned\n  lock and wraps the update in catch_unwind so a pathological file is\n  skipped instead of taking the server down\n- the watcher thread is spawned with an 8 MB stack (it runs tree-sitter,\n  where a stack overflow is an abort, not a panic), matching rayon workers\n- the global rayon pool with 8 MB stacks is built in main before anything\n  can run a par_iter with the 2 MB default\n- the REST handlers (via try_read_engine) and the gRPC search recover from a\n  poisoned read lock instead of returning a permanent 500; WouldBlock still\n  maps to 503 + Retry-After\n\nRoadmap 1.7.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(watcher): handle directory rename/delete and apply build eligibility rules\n\nDirectory events left the index permanently wrong: remove_file(dir)\nmatched no file id and update_file(dir) failed to read a directory, so every\nfile under a renamed or deleted directory stayed indexed under its old path\nand nothing under the new path was indexed.\n\n- new search::incremental::apply_change applies a FileChange to the engine:\n  a path with no file id is treated as a directory (remove_files_under,\n  which matches whole path components against the canonical store paths\n  via a lossy canonicalizer that works for paths that no longer exist), and\n  a directory target is walked with FileDiscoveryIterator\n- the watcher path now uses the same eligibility rules as the initial build\n  (exclude patterns, include_extensions, binary extensions, size cap,\n  exclude_files) through a shared FileDiscoveryIterator::accepts /\n  file_discovery::is_eligible, so a whitelist-configured server no longer\n  indexes a .log file the moment it changes (roadmap 2.4, watcher half)\n- main.rs collapses the three watcher arms into one apply_change call\n\nAdds test_directory_rename_and_delete_update_index and\ntest_watcher_change_respects_eligibility.\n\nRoadmap 1.8.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(api): cap the /api/context window and bound regex compilation size\n\n/api/context accepted any context value: context=usize::MAX overflowed\nmatch_idx + context + 1 (panic in debug, wrapped index in release) and a\nlarge value returned the whole file through the lightweight hover endpoint.\nThe window is now capped at 200 lines each side with saturating arithmetic.\n\nUser regexes are compiled with an explicit RegexBuilder size_limit (4 MB)\nand dfa_size_limit (2 MB), so a pattern like (a{1000}){1000} is rejected\nas a 400 instead of compiling into hundreds of megabytes on a search thread.\n\nAdds test_http_context_caps_window_and_survives_overflow and\ntest_http_regex_size_limit_returns_400.\n\nRoadmap 1.9.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(deps): make dependency-index registration idempotent and removal O(1)\n\nDependencyIndex::remove_file never removed anything from filename_to_paths\n(the loop body was a no-op) and update_file re-registered the path on every\nwatcher event, pushing a duplicate entry each time, so the filename index\ngrew without bound per edited file and a removed path could still be picked\nas a bare-name resolution candidate. path_to_id removal was also a full-map\nretain per event.\n\nAn id -> path map now makes re-registration idempotent (same path: no-op;\nnew path: old lookups dropped first) and removal O(1), pruning the filename\nindex as well.\n\nAdds test_reregister_and_remove_keep_lookups_bounded.\n\nRoadmap 1.11.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(server): bind to loopback by default, opt-in CORS, scope the gRPC Index RPC\n\nThe server has no authentication, the REST API serves full file contents,\nand the gRPC Index RPC indexed any path a client named (and followed\nsymlinks), yet both listeners bound to 0.0.0.0 and CORS allowed any origin.\n\n- default address / web_address are now 127.0.0.1; the config template and\n  README say why and how to expose the server deliberately\n- new server.cors_origins (empty = same-origin only, which the embedded UI\n  needs; explicit origins or \"*\" opt in); create_router keeps its\n  signature and create_router_with_cors takes the list\n- the shipped binary builds the gRPC service with\n  create_server_with_engine_scoped(config.indexer.paths): Index requests\n  outside those canonical roots return PermissionDenied; the walk no longer\n  follows symlinks\n- README gains a \"Network exposure\" section; DEPLOYMENT.md corrected\n\nAdds test_grpc_index_rejects_paths_outside_scope.\n\nRoadmap 1.10.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: tick roadmap 1.7-1.11 (Phase 1 complete) and record changes in CHANGELOG\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(watcher): batch bursts of events and keep ranking caches warm\n\nEach watcher event previously took its own write lock and did a full\nposting-list scan (TrigramIndex::remove_document), invalidated the\nall-documents cache (never rebuilt until the next finalize, so every short\nquery recomputed a union over every posting list), and left the touched\nfile's fast-ranking metadata stale (watcher-added files ranked last; a\nfreshly loaded index had no metadata at all until the background finalize).\n\n- main.rs gathers events for a 200 ms window after the first and applies\n  them with search::incremental::apply_changes under ONE write lock; events\n  are coalesced per path (rename = delete + modify, last op wins)\n- TrigramIndex::remove_documents strips a RoaringBitmap of ids in a single\n  pass; add/remove now update the all-documents cache in place instead of\n  invalidating it\n- SearchEngine::refresh_file_metadata recomputes one file's fast-ranking\n  metadata after update_file and for files whose in-edge count changed on\n  removal; compute_all_file_metadata runs after every persisted load\n\nAdds test_remove_documents_bulk_keeps_cache_warm and\ntest_apply_changes_batches_and_coalesces.\n\nRoadmap 2.1 and 2.2.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs(roadmap): tick 2.1 and 2.2\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(watcher): canonicalize configured roots and match watcher paths exactly\n\nWatcher events arrive under whatever form the root was configured in\n(relative, symlinked, \\\\?\\-prefixed on Windows) while the file store keys on\ncanonical paths, so update_file/remove_file missed the O(1) exact lookup,\nfell back to an O(n) suffix scan (one String per indexed file), and on a\nmiss update_file re-added the file via index_file, which deduped to the\nexisting id and ADDED trigrams without removing the old ones.\n\n- IndexerConfig::canonicalize_paths runs in Config::with_overrides so the\n  watcher and discovery see canonical roots\n- update_file / remove_file use find_file_id_exact (canonical, no suffix\n  fallback); find_file_id keeps the suffix fallback for API lookups only\n\nAdds test_update_file_uses_canonical_exact_match.\n\nRoadmap 2.3.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(indexer): do not queue stale files twice after a checkpoint load\n\nStale files were sent to the batch pipeline explicitly and then sent again\nby the full path scan, because the skip set only held files that were valid\nat load. The discovery thread now remembers what it queued as stale\n(should_skip_discovered), which is also the first unit test in\nbackground_indexer.rs.\n\nRoadmap 2.7.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(deps): per-language import resolution, no bare-name fallback\n\nresolve_import_path treated every import the same way: relative paths were\njoined and probed with a fixed extension list, and anything else fell back\nto \"any file with that filename anywhere in the repo\". Consequences: Rust\n`use crate::a::b` never resolved and `mod foo;` bound to an arbitrary foo.rs;\nPython `from .foo import x` looked for a hidden file `.foo` and dotted\nimports became `foo.bar.py`; `import merge from 'lodash/merge'` linked to any\nlocal merge.ts. The \"heavily imported\" ranking boost inherited all of it.\n\nThe resolver now dispatches on the importing file's language:\n- Rust: crate root (nearest Cargo.toml/src), self/super module directories,\n  longest-prefix probing of a/b.rs and a/b/mod.rs, use-group and glob\n  stripping; external crates resolve to None\n- Python: leading dots walk parent packages, dotted paths are directories,\n  packages resolve to __init__.py, absolute imports search a bounded number\n  of ancestor directories; stdlib names resolve to None\n- JS/TS: relative paths probe the full extension list, .js -> .ts/.tsx,\n  index.*; @/ and ~/ aliases resolve against the nearest package root;\n  bare package names resolve to None\n\nImport extraction gains Python `import a, b as c` (all names), JS\n`export … from` re-exports, dynamic `import()` and backtick require.\n\nAdds test_resolve_rust_module_paths, test_resolve_python_imports,\ntest_resolve_js_imports and test_python_and_js_import_extraction.\n\nRoadmap 2.5.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(deps): retry parked imports only when a matching file appears; persist them\n\nresolve_imports_incremental re-attempted every unresolved import after every\nbatch under the engine write lock. Stdlib/package imports never resolve, so\nthe pending list grew for the whole build and each retry cost up to seven\ncanonicalize syscalls per relative import: O(batches x unresolved). Unresolved\nimports were also dropped by the final resolve_imports and never persisted,\nso a checkpoint restore lost the incoming edge of any file indexed later.\n\n- unresolved imports are parked in waiting_imports, keyed by the lowercase\n  path segments of the import string; a batch retries only the parked\n  imports whose key matches a stem it just added (dir names for mod.rs /\n  __init__.py / index.*), so the cost is proportional to the batch\n- resolve_imports (finalize) makes one last pass and keeps the rest parked\n- parked imports are persisted (PersistedIndex.pending_imports) and\n  re-parked on load with remapped ids; persistence format bumps to v4 /\n  magic FCSIDX02, so older index files are rebuilt cleanly\n\nAdds test_waiting_import_resolves_when_target_appears and\ntest_unresolved_imports_survive_reload; test_pending_imports_count now\nasserts the parked set is stable under repeated resolution.\n\nRoadmap 2.6.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: tick roadmap 2.3-2.7 and record Phase 2 changes\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(discovery): honour .gitignore / .ignore files (indexer.respect_gitignore)\n\nDiscovery used walkdir with only exclude_patterns, so build output outside\nthe eight default excludes (anything a repo lists in .gitignore) was fully\nindexed. Discovery now uses the ignore crate's walker (.gitignore, .ignore,\n.git/info/exclude; hidden files kept, symlinks not followed), and the\nsingle-path check the watcher uses consults the same files via\nis_gitignored, so the initial build and incremental updates agree. New\nconfig indexer.respect_gitignore (default true) switches it off.\n\nAdds test_gitignore_is_respected_and_optional.\n\nRoadmap 2.8.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* fix(index): serve files up to 1 MiB by owned reads, never through a live mmap\n\nA searcher reading a memory-mapped file that an editor truncates in place\ntakes an uncatchable SIGBUS and kills the whole server (the concurrency\ntest added in the next commit reproduced it on the first run). Indexing was\nmoved to owned buffers in 0.9.0; retrieval was not.\n\nFiles at or below MMAP_THRESHOLD_BYTES (1 MiB, i.e. essentially all source\nfiles) are now registered without a mapping and read into an owned buffer\non each access (the page cache makes this cheap; UTF-8 is validated fresh\nevery time). Only larger files keep the zero-copy mapping. This also keeps\nthe number of mappings far below vm.max_map_count on large trees.\n\nget_stats().num_files now reports live (non-tombstoned) files.\n\nAdds test_small_file_survives_concurrent_truncation; the existing store\ntests are updated for the new semantics (with_mmap_failure simulates a\nlarge file).\n\nRoadmap 6.2 (pulled forward from Phase 6 because 2.9's concurrency test\ncrashed on it).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* test: batch pipeline, poisoned-lock recovery, concurrent updates, real watcher\n\n- background_indexer: process_batches drains in batch_size groups and\n  flushes the tail, stops on the shutdown flag, and process_batch recovers\n  a poisoned engine lock instead of dropping the batch\n- integration: searches run in a loop while files are rewritten on disk and\n  update_file is applied (no panics, no duplicate ids, correct final state);\n  a real notify watcher reports create/modify/rename/delete which\n  apply_changes turns into the matching index state\n\nRoadmap 2.9.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: tick roadmap 2.8, 2.9 and 6.2; changelog for mmap and gitignore changes\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(search): one budgeted candidate runner; deterministic order; offset paging\n\nAll four searches (text, filtered text, regex, symbols) previously each\nselected fast/full mode, ordered candidates, ran rayon and truncated on\ntheir own, and Full mode collected every match from every candidate\nbefore truncating: max_results never bounded work.\n\n- run_candidates is now the single path: mode selection, fast-mode\n  ordering by a per-search metadata score, parallel per-document scan\n  under a QueryRun, deterministic sort + paging, eviction\n- SearchLimits carries max_results, offset, a match budget (default\n  8x the page, min 512) and an optional deadline; per-document scans stop\n  materializing matches once the budget is spent and no further documents\n  are opened; SearchRankingInfo reports truncated_by_budget and an exact\n  total_matches when the scan completed\n- ordering ties break on (file_id, line): identical requests return\n  identical pages, and offset paging tiles the full ordering\n- symbol search consults the symbol cache before reading any content and\n  no longer drops low-scoring files before matching\n- evict_all_fallbacks skips small (owned-read) files without locking;\n  the duplicate eviction calls in the REST and gRPC handlers are removed\n- /api/search gains offset and timeout_ms (capped at 30 s), reports\n  total_matches, truncated_by_budget and offset, derives has_more from the\n  real total, and returns ranking info for regex and symbol modes too\n\nAdds test_match_budget_bounds_work_and_is_reported,\ntest_deterministic_order_and_offset_paging and\ntest_http_search_offset_paging_and_totals.\n\nRoadmap 3.1, 3.2, 3.5 (runner half), 3.6, 3.8 (partial).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(regex): accelerate (?i) literals and repeated suffixes; cache compiled regexes\n\nTrigram constraints were extracted only from HirKind::Literal, so\n(?i)needle (which compiles to per-character case classes) and abc+ (whose\nmandatory c is a Repetition) produced no constraint and fell back to a\nfull corpus scan with a warn! per query. The project's own validator uses\n(?i). Constraint extraction now works on \"mandatory text\": literals,\nsingle-character case-insensitive classes (lowered to the lowercase char\nthe lowercased index needs), and the first copy of a min>=1 repetition,\nmerged across a concatenation.\n\nCompiled analyses are kept in a 64-entry LRU on the engine so\nsearch-as-you-type does not recompile the same pattern per keystroke. The\ndead RegexAnalysis::literals / best_literal / extract_literals_recursive\nare removed and the regex tests rewritten against constraints.\n\nAdds test_case_insensitive_literal_is_accelerated and\ntest_repetition_prefix_is_required.\n\nRoadmap 3.3.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs(roadmap): tick 3.1-3.3, 3.6; note 3.5/3.8 partial\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(search): whole-buffer ASCII scan, lazy symbol maps, precomputed display paths\n\n- plain-text verification scanned every line of every candidate with a\n  scalar byte loop; ASCII needles now use memchr2 on the first byte (both\n  cases) over the whole buffer and resolve line bounds only at hits\n  (identical results to the per-line search, CRLF and last-line included;\n  non-ASCII needles keep the Unicode per-line path)\n- per-document symbol maps (definition lines, names by line) were built\n  eagerly for every candidate; they are now built on the first hit\n  (SymbolLineMaps), so trigram candidates that fail verification cost\n  nothing beyond the scan\n- FileMetadata carries the root-relative display path, so include/exclude\n  filtering and result construction no longer allocate a String per\n  candidate\n\nAdds test_ascii_line_hits_matches_per_line_search.\n\nRoadmap 3.4.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(api): full-line match offsets and character column on every result\n\nmatch_start/match_end are byte offsets into the possibly truncated content\nwindow, but consumers (the VS Code provider among them) applied them as\ncharacter columns on the real line. Results now also carry\nline_match_start / line_match_end (bytes into the full line; into the\ndisplay path for filename hits) and match_column (0-based character\ncolumn), on the engine, REST and gRPC (proto fields 9-11) surfaces. The\ndocs page documents the contract plus offset / timeout_ms /\ntotal_matches / truncated_by_budget from the previous commits.\n\nAdds test_match_offsets_refer_to_full_line.\n\nRoadmap 3.7.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs(roadmap): tick 3.4 and 3.7\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* refactor(search): RankingWeights/FileScoreWeights; symbol exact>prefix>substring, one row per line\n\nAll boosts (exact case, definition, src/lib, line start, length floor,\nfilename hit, dependency log10 scale) and the file-level fast-ranking\nterms now live in search::ranking with documented defaults, replacing\nmagic numbers scattered across three per-document scorers and\nFileMetadata::compute; the three copies of the dependency-boost formula\nare one function. The file-level and line-level dependency terms are on\ndifferent scales by design (documented: the file score only orders which\nfiles fast mode opens and never appears in a result).\n\nSymbol search ranks exact name > prefix > substring (2.0 / 1.5 / 1.0),\nweights variables/constants slightly below type and function definitions,\nand emits one row per line, keeping the best score when several symbols on\na line match.\n\nAdds test_symbol_search_ranking_and_line_dedupe.\n\nRoadmap 3.5 (weights) and 3.8.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: Phase 3 complete in roadmap and changelog\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(api): resolve display paths through their root; read context files once per request\n\nEvery path-addressed endpoint took an O(n) suffix scan with a String per\nindexed file because the UI round-trips display paths\n(<root name>/<relative>) that never matched the canonical map.\nfind_file_id now reverses make_display_path onto the configured roots\n(O(roots), exact lookup), which is also unambiguous when two roots contain\nthe same relative path; the suffix scan remains only as a last resort.\n\nThe search handler used find_file_id per result to fetch context lines and\nre-read the file for every hit; it now uses the result's file id and\nsplits each file once per request.\n\nAdds test_find_file_id_by_display_path_across_roots.\n\nRoadmap 4.1.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(grpc): align Search with the REST contract\n\n- max_results = 0 (proto3 unset) now means the default page of 50 instead\n  of being clamped to a single result\n- new request fields: offset (deterministic paging), rank (auto/fast/full,\n  honoured for regex too) and deadline_ms (capped at 30 s); the handler uses\n  the same budgeted *_with_limits engine calls as /api/search\n- results carry dependency_count (field 12) alongside the full-line match\n  offsets; SYMBOL_REFERENCE is documented as reserved\n- the instrumented span's query/max_results fields are actually recorded\n\nAdds test_grpc_search_defaults_offset_and_fields; examples and tests use\n..Default::default() for SearchRequest so future fields do not break them.\n\nRoadmap 4.2.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(server): request limits, search concurrency cap, readiness and metrics\n\n- REST: TimeoutLayer (server.request_timeout_secs, default 30 s) and a\n  64 KB body limit; searches take a permit from a Semaphore sized by\n  server.max_concurrent_searches (default 64) and get 503 + Retry-After\n  when none is free, instead of piling up on tokio's blocking pool and\n  stalling every other endpoint\n- gRPC: per-request timeout and per-connection concurrency limit; the\n  standard grpc.health.v1 service is registered; the trace span carries\n  the request path\n- /api/ready answers 200 only when the index can serve (build completed,\n  or an index is loaded with no build running); /api/health stays liveness\n- /metrics exposes Prometheus text: search request/error counters by\n  reason, a latency histogram, and index gauges (files, trigrams, edges,\n  bytes, indexing, ready) — hand-rendered, no new dependency\n- RouterOptions (from ServerConfig) bundles CORS, limits and timeout for\n  create_router_with_options; older constructors keep working\n\nAdds test_http_ready_and_metrics and test_http_search_concurrency_limit.\n\nRoadmap 4.3 and 4.4.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs(roadmap): tick 4.1-4.4\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(config): reject unknown keys, validate at startup, --web-address; JSON query errors\n\nConfig: all four config structs use deny_unknown_fields (a typo such as\nexlude_patterns used to be silently ignored); Config::validate runs after\noverrides and errors on unparseable or duplicate addresses, zero limits and\na missing index_path directory, warning about empty/missing index paths;\nnew --web-address CLI flag; OTEL_SDK_DISABLED=true is now final (it could\nbe re-enabled by FCS_TRACING_ENABLED); a warning is logged when RUST_LOG\noverrides --verbose. DEPLOYMENT.md documents the environment variables the\nserver actually reads instead of two that never existed.\n\nAPI: ApiQuery wraps axum's Query extractor so a malformed parameter\n(max=abc) returns the standard JSON {\"error\": …} 400; the progress\nWebSocket pings every 30 s and the broadcast buffer grows to 64.\n\nAdds test_unknown_key_rejected_and_template_parses,\ntest_validate_reports_errors_and_warnings,\ntest_with_overrides_sets_web_address and\ntest_http_bad_query_param_is_json_error.\n\nRoadmap 4.5 and 4.8.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(server): real diagnostics config with cached breakdown; lock-light gRPC Index\n\n/api/diagnostics reported a hard-coded config (\"see server configuration\",\n10 MB, watch=false) and walked every file, allocating a String each, on\nevery call. The router now carries the IndexerConfig (RouterOptions\n.indexer_config) and reports it via ConfigSummary; the extension breakdown\nis cached per engine generation (a counter bumped on every mutation) and\nrecomputed only when the index changed or force_refresh=true; sampling\npicks ids first and builds paths only for the sampled files.\n\nThe gRPC Index RPC held the engine write lock for its entire directory\nwalk and ignored every eligibility rule. It now discovers files without\nthe lock using the indexer's FileDiscoveryIterator (excludes, include\nextensions, size cap, .gitignore, exclude_files), processes each batch in\nparallel outside the lock and merges under a short write lock per batch,\nthen resolves imports and finalizes. The dead new_with_indexing /\ncreate_server / create_server_with_indexing / create_indexed_engine paths\n(which still carried the substring-exclusion bug) are removed;\ncreate_server_with_engine_config is what the binary uses.\n\nThe scoped-Index test now also checks that node_modules is excluded.\n\nRoadmap 4.6 and 4.7.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* test(api): diagnostics reports the configured indexer settings\n\nRoadmap 4.9.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: Phase 4 complete in roadmap and changelog\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(symbols): extract via the grammars' tags queries; more kinds and coverage\n\nSymbol extraction was a single 13-language match on node kinds with\ncolliding names; adding a language meant editing that arm list and\npositions had only just been fixed. Each grammar's own tags.scm query\n(TAGS_QUERY; the C# one is vendored because its crate does not export it)\nnow drives extraction, with @name giving the position and the\n@definition.* capture the kind, refined by node kind where a query lumps\nconstructs together (Go type_spec, Rust struct/enum/type, PHP traits, C++\nmember declarations). The hand-written walker still runs afterwards and\nadds anything the queries miss (Rust consts and trait signatures, C#\nproperties/delegates, Go type aliases, C++ in-class declarations, Bash);\nresults are merged on (name, line).\n\n- new SymbolType variants: Module, Macro, Field, Property (C# properties\n  were typed Method; namespaces and Ruby modules are Module now)\n- C/C++ pointer and reference declarators are unwrapped to the identifier\n- .h headers that are unmistakably C++ use the C++ grammar; extensions\n  match case-insensitively; .inl/.phtml added\n- one tree-sitter Parser per thread, re-targeted per file, with a 2 s\n  parse timeout (a cancelled parse indexes the file without symbols)\n- JSON/TOML/YAML/HTML/CSS/Markdown are no longer parsed at all (no\n  captures ever existed for them) and their grammar crates are dropped\n\nAdds test_python_extraction_with_lines, test_c_and_cpp_header_extraction\nand test_extra_kinds_and_coverage; Ruby modules and C++ namespaces now\nassert Module.\n\nRoadmap 5.1-5.6.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: Phase 5 complete in roadmap and changelog\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(index): fold case during trigram extraction; bitset dedupe\n\nTrigram extraction lowercased the whole buffer into a second String and\nthen did one hash-set insert per byte (with the set capacity capped at\n1024, so it rehashed repeatedly). ASCII content (the common case) is now\nfolded per byte during extraction with no copy, and uniqueness is tracked\nin a per-thread 2^24-bit bitset (only touched bits are cleared), so the\nhash set is built once from the unique trigrams. Non-ASCII content still\ngoes through to_lowercase() so multi-byte case mappings match the query\nside exactly.\n\nAdds test_lowercase_extraction_matches_reference. Neutral on the 3 KB-file\nbenchmark (tree-sitter dominates there); the win is on large files.\n\nRoadmap 6.4 (extraction half).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(symbols): single-cursor traversal for the supplementary walker\n\nRoadmap 5.5.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(symbols): disable the reference/doc patterns of each tags query\n\nOnly patterns that capture a @definition.* stay enabled; @reference.call on\nevery call expression and the @doc comment patterns matched far more nodes\nthan the definitions and were ignored. Repeated benchmark runs put indexing\nat the pre-tags baseline (~12 ms for the 25-file fixture).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* build: release profile, semantic feature gate, dependency cleanup\n\n- [profile.release]: thin LTO, codegen-units = 1, strip = true\n- the semantic engine (src/semantic*, ndarray, hnsw_rs, sha2) is behind a\n  new `semantic` feature, off by default; `ml-models` implies it; the\n  semantic binary and example declare required-features. The keyword\n  server build no longer compiles the vector index.\n- md5 dropped: the config fingerprint uses FxHasher (documented,\n  deterministic); the semantic web ETag uses rust-embed's compile-time\n  sha256 like the keyword UI already did. One-time index rebuild on the\n  first start after upgrading (fingerprint format changed).\n- glob dropped (globset remains); sysinfo 0.33 -> 0.39; criterion\n  0.5 -> 0.8 (benches use std::hint::black_box)\n- deny.toml + a cargo-deny CI step (advisories, licenses, sources);\n  Dependabot for cargo and GitHub Actions; CI also runs the lib tests with\n  the semantic feature\n\nRoadmap 6.6 (except the OpenTelemetry bump).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* build: opentelemetry 0.27 -> 0.32 (single tonic/axum/prost stack)\n\nopentelemetry-otlp 0.27 pulled in tonic 0.12, axum 0.7, prost 0.13 and rand\n0.8 next to the project's 0.14 / 0.8 / 0.14 / 0.9. telemetry.rs is\nrewritten for the 0.32 API (SdkTracerProvider with a batch exporter, a\nkept provider handle for shutdown since the global shutdown hook is gone).\nThe dependency graph shrinks from 461 to fewer packages with no duplicate\ntonic or axum.\n\nRoadmap 6.6 (complete).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: tick 6.4 and 6.6; changelog for build and extraction changes\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(index): persistence v5 — nanosecond mtimes, byte paths, run-optimized bitmaps\n\n- staleness compares nanosecond mtimes (a same-size edit within one second\n  was invisible); the value recorded at read time is nanoseconds too\n- file paths are persisted as raw bytes (lossless on Unix), so one\n  non-UTF-8 filename no longer makes every checkpoint fail\n- posting lists are run-optimized in finalize(), so ubiquitous trigrams\n  cost bytes instead of a bitmap container per 65k documents, on disk and\n  in memory\n- magic FCSIDX03 / version 5: older files are rejected before decoding\n  with a clear \"will be rebuilt\" message\n\nAdds test_load_rejects_older_magic and\ntest_non_utf8_path_round_trips_and_nanosecond_mtime. The sectioned,\nmmap-able layout (lazy load, no double materialization) remains open.\n\nRoadmap 6.1 (partial).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(search): query syntax — file:/lang:/-term/case:/word:, phrases, AND terms\n\nPlain-text queries now understand a small Zoekt-style syntax\n(search::query_syntax): quoted phrases, several terms that must all appear\nin a file (lines matching any are returned), -term to drop files containing\na term, file:/-file: path globs, lang:/-lang: by language name or\nextension, and case:yes / word:yes. Candidates are the intersection of the\nterms' trigram sets; verification uses a new options-aware line scanner\n(exact memmem for case-sensitive, memchr2 fold for ASCII, Unicode fold\notherwise) with word-boundary checks that understand multi-byte letters.\nSymbol search honours the globs and compares names exactly / whole when\nasked.\n\nREST: q is parsed by default (regex mode is untouched); case=true|false\nand word=true|false override the in-query switches. gRPC: SearchRequest\ngains case_sensitive and whole_word. The docs page documents the syntax.\n\nAdds query_syntax unit tests, test_query_syntax_search,\ntest_line_hits_options and test_http_query_syntax.\n\nRoadmap Phase 7 (multi-line regex and SYMBOL_REFERENCE results remain\nopen).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: Phase 7 in roadmap and changelog\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* chore(index): delete the dead FileStore/MappedFile module\n\nOnly its re-export referenced it; the engine has used LazyFileStore since\nthe lazy-load rewrite. 414 lines and 12 tests of dead code removed.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* refactor(search): split engine.rs into engine/{mod,text,query,persist,progress,tests}.rs\n\nengine.rs had grown to 5.8k lines holding text matching helpers, the query\nrunner, persistence/reconciliation, progress types and 1.4k lines of\ntests. It is now a directory module: mod.rs keeps the types, indexing and\nincremental-update methods; text.rs the matching/scoring helpers; query.rs\nthe runner and per-document scans; persist.rs save/load/reconcile and\nsymbol rebuild; progress.rs the status/progress types; tests.rs the unit\ntests. No behaviour change; public paths are unchanged.\n\nRoadmap cross-cutting (engine.rs split).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: archive stale review/plan, refresh CONTRIBUTING, add PR template, CODEOWNERS, SECURITY\n\n- docs/REVIEW.md (a review of v0.2.1) and the completed June 2026 plan move\n  under docs/archive/\n- CONTRIBUTING's \"Areas for Contribution\" listed work shipped long ago; it\n  now points at the open roadmap items\n- .github/PULL_REQUEST_TEMPLATE.md, CODEOWNERS and SECURITY.md added\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: rewrite DEVELOPMENT.md against the code; roadmap status and cross-cutting ticks\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* build: keep sysinfo on the latest release that supports the 1.89 MSRV\n\nsysinfo 0.39 requires rustc 1.95; the MSRV job caught it.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: describe persisted index format v5 in the changelog\n\nThe entry still said v4 / FCSIDX02; the branch writes FCSIDX03 with\nnanosecond mtimes, byte paths and run-optimised bitmaps.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* build: clear cargo-deny advisories\n\ncargo-deny in CI failed on the RustSec database as of 2026-09-05:\n\n- rustls-webpki 0.103.9 -> 0.103.15 (RUSTSEC-2026-0049/0098/0099/0104)\n- h2 0.4.13 -> 0.4.19 (RUSTSEC-2026-0258)\n- bytes 1.11.0 -> 1.12.1 (RUSTSEC-2026-0007)\n- memmap2 0.9.9 -> 0.9.11 (RUSTSEC-2026-0186)\n- crossbeam-epoch 0.9.18 -> 0.9.20 (RUSTSEC-2026-0204)\n- anyhow 1.0.100 -> 1.0.104 (RUSTSEC-2026-0190)\n- rand 0.9.2 -> 0.9.5 (RUSTSEC-2026-0097)\n- indicatif 0.17 -> 0.18, which drops the unmaintained number_prefix\n  (RUSTSEC-2025-0119)\n\nTwo unmaintained-crate advisories are ignored with reasons in deny.toml:\nbincode 1.x is the pinned index codec (migration is roadmap item 6.1) and\npaste is a compile-time proc-macro reached only via the optional semantic\nfeature. MSRV 1.89 still checks with the updated lock.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* test: make the query-syntax exclusion test independent of the temp path\n\n`-file:a` is a substring match over the whole display path, and with no\nroot registered that is the absolute temp path. macOS temp dirs live\nunder /var/folders and Windows under AppData, so every file was excluded\nand the macOS CI job failed (Linux only passed when the random temp name\nhappened not to contain an 'a'). Exclude by file name instead.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-05T09:15:40+01:00",
          "tree_id": "ac148729a502e0ccd4642c33f0d4677ee7f5a887",
          "url": "https://github.com/jburrow/fast_code_search/commit/1b99b6a609fcb9312846d8e9c6b9b937dc754659"
        },
        "date": 1788596860771,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 253831,
            "range": "± 24912",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 21950,
            "range": "± 580",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 443,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 255649,
            "range": "± 8177",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 20841,
            "range": "± 669",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 558,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 250900,
            "range": "± 7311",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 21784,
            "range": "± 779",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 778,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 420399,
            "range": "± 13274",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 357980,
            "range": "± 8897",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 491277,
            "range": "± 13003",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 581552,
            "range": "± 12189",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 251749,
            "range": "± 8619",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 264081,
            "range": "± 4220",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 326748,
            "range": "± 11520",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 420073,
            "range": "± 4160",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 248677,
            "range": "± 5776",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 247338,
            "range": "± 7170",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 296927,
            "range": "± 10360",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 175663,
            "range": "± 3483",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 247335,
            "range": "± 5856",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 547000,
            "range": "± 12151",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 379674,
            "range": "± 13164",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 344388,
            "range": "± 11754",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 7311,
            "range": "± 29",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 9961305,
            "range": "± 12391",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 19760579,
            "range": "± 137633",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 39198755,
            "range": "± 54112",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 21265720,
            "range": "± 67176",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 23893149,
            "range": "± 72598",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 41907738,
            "range": "± 71725",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 47622187,
            "range": "± 62058",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 745751,
            "range": "± 1274606",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 1503144,
            "range": "± 1477138",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 2821063,
            "range": "± 3672585",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1404694,
            "range": "± 44563",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 5192356,
            "range": "± 21043",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 9891990,
            "range": "± 27959",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 126657,
            "range": "± 3730",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 173441,
            "range": "± 3622",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 228863,
            "range": "± 4701",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 103612,
            "range": "± 1197",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 475998,
            "range": "± 3164",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 945807,
            "range": "± 7612",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3366521d241752f2df6d73c1b586dc72302b1dc9",
          "message": "Keyword roadmap 3 (#116)\n\n* ci: stop shipping debug archives with releases\n\nRoadmap 6.6 leftover. Each release built and uploaded a second,\nunoptimised binary per target. Release binaries are LTO'd and stripped;\nanyone debugging builds from the tag.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(deps): resolve import candidates lexically, never through realpath\n\nEvery candidate path the resolver probed that was not an exact map hit\nwas canonicalized, i.e. a readlink per path component. Python absolute\nimports probe up to eight ancestor directories with two candidates per\nsegment, so building a 6k-file tokio+django corpus made 17.6 million\nfailing readlink calls: 25 s of build, 32 s of system time.\n\nCandidates are joined onto registered canonical paths, so resolving\n'.' and '..' lexically and looking the result up in the map answers\nthe same question. Build time 25.5 s -> 8.7 s on that corpus with the\nidentical 8,466 edges; register_file also skips the realpath walk when\nthe engine re-registers a path it already holds.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(index): parallel single-document removal from the posting lists\n\nRoadmap 6.5 finding. Updating one file from the watcher cost 15.6 ms on\na 6k-file corpus, 13 ms of it in remove_document: a sequential retain\nover 134k posting lists doing a bitmap intersection on each. The scan is\nnow parallel and the single-document path only pays a membership test.\nPosting lists left empty are pruned by finalize instead of on every\nremoval. One-file update: 15.6 ms -> 5.2 ms (removal 13.4 ms -> 2.7 ms).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(watch): cache compiled gitignore matchers per directory\n\nRoadmap 6.5 finding. Every watcher event rebuilt the .gitignore/.ignore\nmatcher (parse + one regex per pattern) for each ancestor directory of\nthe changed file, about 10 ms per event on a repository with a normal\nroot .gitignore. Matchers are now cached per directory, keyed by the\nignore files' modification times so an edited ignore file is re-read.\nOne-file update on the tokio+django corpus: 16 ms -> 4.3 ms.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* bench: real-corpus benchmark in CI, nightly large-corpus run\n\nRoadmap 6.5. examples/corpus_bench.rs indexes real directories the way\nthe server does and reports build throughput, resident memory, query\nlatency percentiles (text, regex, symbol, filtered), incremental update\ncost and index save/load, as a markdown table and optionally as bencher\nlines. The benchmark workflow checks out tokio 1.45.0 and Django 5.2\n(pinned, shallow) on every push to main, feeds the metrics into the\nexisting trend chart and the job summary, and a nightly job runs the\nsame over rust-lang/rust 1.89.0. The README benchmark section carries\nthe measured table with provenance.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(search): multi-line regex\n\nRoadmap Phase 7. A pattern that mentions a newline (\\n, \\r, \\x0a) or sets\nthe s flag ((?s)begin.*?end) is matched against the whole file content\ninstead of line by line. Each match is reported once, on the line where\nit starts, with the in-line offsets clamped to that line; the per-document\ncap and the query budget apply as before. Patterns without either marker\nare unchanged, so \\s+ still never crosses a line.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(index): compact once, in finalize\n\nRoadmap 6.4 leftover. The batch loop called compact_memory every 50\nbatches, which shrank the trigram hash map to fit and forced the next\nbatch to grow and rehash it again. finalize already releases capacity\nonce at the end; the periodic call and the now-unused method are gone.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* chore: remove unreferenced public functions\n\nA whole-tree scan (src, tests, benches, examples) found nine pub fns\nwith no reference anywhere: IndexerConfig::is_path_in_scope,\nDependencyIndex::files_with_dependents, LazyMappedFile::new_small,\nTrigram::from_slice, TrigramIndex::num_documents,\ndiscover_files_with_config, PathFilter::filter_documents_by_display,\nSystemLimits::can_allocate_more and diagnose_mmap_error.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* test(web): CORS headers and /ws/progress\n\nExplicit coverage for two serving behaviours that had none:\nserver.cors_origins (default: no Access-Control-Allow-Origin at all; a\nlisted origin is echoed, an unlisted one is not, preflight succeeds,\n\"*\" allows any) and the progress WebSocket (101 handshake with\nSec-WebSocket-Accept, an initial status frame on connect, a broadcast\nupdate relayed). The WebSocket test speaks the handshake and frame\nformat directly so no client dependency is added.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(symbols): symbol references (call sites, type mentions)\n\nRoadmap Phase 7. The grammars' tags queries already describe references\n(@reference.call, @reference.class, @reference.implementation, ...);\nthose patterns are now kept instead of disabled, and the Rust query is\nsupplemented with path calls (crate::run(), Type::new()) and generic\ncalls, which upstream leaves out.\n\nReferences are stored apart from definitions: per file, a list of\n(interned name id, line, column) at 12 bytes each, with one name table\nper engine. They are persisted (format v6, magic FCSIDX04; older files\nare rebuilt), remapped on save/load like symbols, refreshed by update\nand cleared by removal.\n\nSearchEngine::search_references(name) returns the lines where the\nidentifier is used (exact, case-sensitive; one result per line; the\ntrigram index pre-filters candidates so only files containing the text\nhave their reference lists scanned). REST: /api/search?references=true;\ngRPC: SearchRequest.references. Results carry match_type\nSYMBOL_REFERENCE, which the web UI already labels.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: reference search parameter and results\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* bench: report symbol references in the corpus benchmark\n\nAdds a reference-search query and the reference count to\nexamples/corpus_bench.rs; README numbers updated (references add about\n6 MB of memory and 3 MB on disk for 220k references on tokio + Django,\nand no measurable build time in an alternating A/B against the\npre-reference binary).\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* feat(index): sectioned, checksummed persisted index (format v7)\n\nRoadmap 6.1. The index file is now a 36-byte header (magic FCSIDX05,\nversion, CRC-32, section lengths) followed by three sections: the\nmetadata (bincode of everything but the posting lists), a fixed-width\ntrigram directory sorted by trigram, and the roaring bitmaps back to\nback. The CRC covers all three sections.\n\nSave streams: bitmaps are serialized straight from the live map into\nthe file (no per-bitmap Vec, no serialized copy of the whole index).\nLoad memory-maps the file, validates magic, version, section bounds and\nchecksum before decoding anything, decodes the metadata, and\ndeserializes bitmaps in parallel directly out of the mapping.\nReconciling load of the 22 MB tokio+django index: 0.31 s -> 0.21 s.\n\nA golden fixture (tests/fixtures/index-v7.fcsidx) pins the format: a\ntest asserts that saving a fixed index reproduces it byte for byte and\nthat it loads with every section intact; corrupt bodies and bad section\nlengths are rejected with a rebuild message. Older magics (v1..v6) are\nrejected up front. Fixtures are marked binary in .gitattributes.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: describe the v7 index layout; tick roadmap 6.1\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* docs: roadmap status: complete\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* ci: surface test crashes as annotations; reset Criterion state before benchmarks\n\nThe macOS test job fails with exit code 101 and no failed-test line, so\nthe annotation step had nothing to echo. The log scraper now also\nreports panics, cargo's \"process didn't exit successfully\" / signal\nlines and compile errors, and falls back to the last 40 log lines. CI\nalso gains workflow_dispatch so a run can be triggered on main.\n\nThe benchmark job failed at the trend-tracking step: rust-cache prunes\nfiles it does not recognise, leaving Criterion baseline directories\nwithout sample.json, and Criterion's error text broke the bencher lines.\ntarget/criterion is removed before benchmarks run, --noplot is passed,\nand only well-formed bencher lines are handed to the action.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n* perf(index): load a 61k-file index in 8 s instead of 23 s; phase timings in the log\n\nMeasured on a real 61k-file / 545 MB workspace index (external HDD):\n\n- Restoring symbols and imports registered every file with the\n  dependency index through realpath, one walk per file: 15.9 s -> 0.3 s\n  by registering the store's already-canonical path\n  (register_canonical_file; the batch merge and update paths use it too).\n- The staleness check stat'ed each file twice (exists, then metadata):\n  one stat now answers both (7.9 s -> 3.5 s).\n- The \"Index loaded from disk\" line carries total_ms and per-phase\n  timings so the next slow load says which step it was.\n\nAlso: 5xx responses are traced at debug rather than error, since a\nreadiness probe answering 503 during the build is expected and the\nhandlers log genuine failures themselves.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-06T09:55:13+01:00",
          "tree_id": "f3afd037063b030cd4e381f3ff0e6d2d2d4bfda7",
          "url": "https://github.com/jburrow/fast_code_search/commit/3366521d241752f2df6d73c1b586dc72302b1dc9"
        },
        "date": 1788685591810,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 328982,
            "range": "± 24935",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 31866,
            "range": "± 904",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 414,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 333402,
            "range": "± 4476",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 32152,
            "range": "± 868",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 420,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 329944,
            "range": "± 11083",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 32595,
            "range": "± 978",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 422,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 536701,
            "range": "± 10362",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 459174,
            "range": "± 9631",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 623233,
            "range": "± 9444",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 750222,
            "range": "± 11185",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 331672,
            "range": "± 8106",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 355089,
            "range": "± 3554",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 410946,
            "range": "± 6392",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 552550,
            "range": "± 5784",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 331098,
            "range": "± 8434",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 327248,
            "range": "± 8101",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 379115,
            "range": "± 6852",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 230288,
            "range": "± 4558",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 332324,
            "range": "± 15041",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 702528,
            "range": "± 22669",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 482216,
            "range": "± 12105",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 447859,
            "range": "± 12054",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 8349,
            "range": "± 32",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 14285127,
            "range": "± 42032",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 28443579,
            "range": "± 62851",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 56333641,
            "range": "± 195129",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 25447563,
            "range": "± 63167",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 27403090,
            "range": "± 151857",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 50500083,
            "range": "± 319733",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 54403550,
            "range": "± 193134",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1212253,
            "range": "± 84593",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3564238,
            "range": "± 93934",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6519575,
            "range": "± 387256",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1250840,
            "range": "± 13317",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 4228719,
            "range": "± 31714",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 7941267,
            "range": "± 121252",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 163887,
            "range": "± 4774",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 223501,
            "range": "± 6386",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 281823,
            "range": "± 6685",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 73679,
            "range": "± 1461",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 307512,
            "range": "± 2866",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 598803,
            "range": "± 119908",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 4335181574,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 181051359,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 207876349,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 890219,
            "range": "± 74830",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 1078261,
            "range": "± 335289",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 501,
            "range": "± 100",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 1237169,
            "range": "± 45015",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 865643,
            "range": "± 59181",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 896010,
            "range": "± 51466",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 1884583,
            "range": "± 985177",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 1721778,
            "range": "± 242875",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 1901425,
            "range": "± 331592",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 3338278,
            "range": "± 485911",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 3567408,
            "range": "± 441178",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 606497,
            "range": "± 57598",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 5451430,
            "range": "± 3240295",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 122097664,
            "range": "± 0",
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
          "id": "5507264393f4955bedde8a4ed0785f9a5417d866",
          "message": "ci: let the test-failure annotations actually run\n\nGitHub executes bash steps with -e, so a failing cargo test pipeline\naborted the step before the annotation script ran; that is why the\nmacOS and Windows failures showed only \"exit code 101\". The pipeline's\nstatus is now captured with || instead.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-06T10:08:02+01:00",
          "tree_id": "abb7776fe6c36a3c8352bc822af21ce1b7bb6ec5",
          "url": "https://github.com/jburrow/fast_code_search/commit/5507264393f4955bedde8a4ed0785f9a5417d866"
        },
        "date": 1788686265472,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 191735,
            "range": "± 16737",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 15300,
            "range": "± 603",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 228,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 183810,
            "range": "± 5550",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 15030,
            "range": "± 1022",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 217,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 186346,
            "range": "± 5035",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 15465,
            "range": "± 800",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 234,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 330880,
            "range": "± 8897",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 284890,
            "range": "± 12542",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 378820,
            "range": "± 8776",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 471759,
            "range": "± 14107",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 199677,
            "range": "± 5057",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 186807,
            "range": "± 3282",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 230090,
            "range": "± 6501",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 303064,
            "range": "± 6805",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 192495,
            "range": "± 5769",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 184333,
            "range": "± 6466",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 238596,
            "range": "± 14895",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 130765,
            "range": "± 4081",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 197209,
            "range": "± 6223",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 406349,
            "range": "± 10175",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 288381,
            "range": "± 8913",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 266410,
            "range": "± 12794",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 5863,
            "range": "± 151",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 7129810,
            "range": "± 315661",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 14010484,
            "range": "± 128146",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 28412114,
            "range": "± 608464",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 13372435,
            "range": "± 58406",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 13542958,
            "range": "± 137454",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 25526724,
            "range": "± 152374",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 27088144,
            "range": "± 310187",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1597954,
            "range": "± 1541166",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 4524621,
            "range": "± 6305643",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6643479,
            "range": "± 4483197",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 763354,
            "range": "± 56262",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 2735312,
            "range": "± 97898",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 5226733,
            "range": "± 225222",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 95564,
            "range": "± 4152",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 130387,
            "range": "± 4587",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 165370,
            "range": "± 3476",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 47633,
            "range": "± 789",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 206006,
            "range": "± 3167",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 402316,
            "range": "± 6541",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 2471646843,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 260346170,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 151779722,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 531793,
            "range": "± 136503",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 913050,
            "range": "± 128352",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 271,
            "range": "± 79",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 703608,
            "range": "± 39439",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 498943,
            "range": "± 78708",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 525359,
            "range": "± 49587",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 1083094,
            "range": "± 147660",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 939230,
            "range": "± 171085",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 1192367,
            "range": "± 148402",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 1960872,
            "range": "± 302040",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 2134911,
            "range": "± 263613",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 361609,
            "range": "± 163033",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 4597887,
            "range": "± 1658296",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 125018112,
            "range": "± 0",
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
          "id": "79ee2be2e0fb59b75dcf181ea92cc25bcad3e51f",
          "message": "fix(deps): resolve imports from a non-canonical importing path\n\nThe lexical resolver (no realpath per candidate) assumed the importing\npath was already canonical. Any caller passing the path as discovered\nbroke on platforms where that differs from the canonical form: macOS\ntemp dirs sit behind the /var -> /private/var symlink and Windows\ncanonical paths carry the \\\\?\\ prefix, which is why seven import\nresolution tests failed on both CI runners while passing on Linux. The\nimporting path is now canonicalized once per file when it is not a\nknown key, never per candidate. Regression test resolves through a\nsymlink.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-06T10:43:52+01:00",
          "tree_id": "7a903dc3aafb014f790e95a926ec3f7b40f8e28c",
          "url": "https://github.com/jburrow/fast_code_search/commit/79ee2be2e0fb59b75dcf181ea92cc25bcad3e51f"
        },
        "date": 1788688501608,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 330028,
            "range": "± 17542",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 31487,
            "range": "± 947",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 414,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 327070,
            "range": "± 6539",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 30945,
            "range": "± 1072",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 415,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 329500,
            "range": "± 6153",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 30679,
            "range": "± 1592",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 423,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 530568,
            "range": "± 9857",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 461461,
            "range": "± 7104",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 614359,
            "range": "± 11500",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 742033,
            "range": "± 11405",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 328811,
            "range": "± 6792",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 344581,
            "range": "± 18736",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 408109,
            "range": "± 7063",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 545193,
            "range": "± 7067",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 325140,
            "range": "± 9365",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 324812,
            "range": "± 5731",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 374706,
            "range": "± 11254",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 228023,
            "range": "± 3657",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 323526,
            "range": "± 7069",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 688737,
            "range": "± 10469",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 474524,
            "range": "± 15166",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 446253,
            "range": "± 10960",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 8178,
            "range": "± 23",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 14216453,
            "range": "± 79174",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 28158009,
            "range": "± 60690",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 55831091,
            "range": "± 57491",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 25434450,
            "range": "± 64400",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 27101786,
            "range": "± 43676",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 49946135,
            "range": "± 231386",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 53837642,
            "range": "± 518647",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1174447,
            "range": "± 32564",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3486460,
            "range": "± 54921",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6344213,
            "range": "± 69079",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1231061,
            "range": "± 13496",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 4189060,
            "range": "± 29995",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 7865941,
            "range": "± 36978",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 164046,
            "range": "± 4090",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 213539,
            "range": "± 5586",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 274093,
            "range": "± 8128",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 73133,
            "range": "± 1956",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 303023,
            "range": "± 2645",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 593171,
            "range": "± 4635",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 4162744193,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 169578504,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 194019130,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 871114,
            "range": "± 87073",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 1356199,
            "range": "± 88846",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 491,
            "range": "± 80",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 1211048,
            "range": "± 89678",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 777660,
            "range": "± 158465",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 896321,
            "range": "± 39024",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 1811028,
            "range": "± 558230",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 1572953,
            "range": "± 68117",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 1950757,
            "range": "± 388776",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 3081085,
            "range": "± 358539",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 3358993,
            "range": "± 167555",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 595971,
            "range": "± 248463",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 3594022,
            "range": "± 2732786",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 120295424,
            "range": "± 0",
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
          "id": "ec5d6f908a26b1b90a0d0bdb9e47a497c8e80275",
          "message": "test(watch): settle on end state per step in the e2e watcher test\n\nFSEvents on macOS coalesces events and can deliver the halves of a\nrename late and unpaired, so the first event seen after a disk\noperation may belong to the previous one; the delete step then applied\na stale event for the old name and asserted before the real Remove\narrived (the last remaining macOS CI failure). Each step now keeps\ndraining, applying and re-checking the expected index state for up to\n8 s, which is what a real consumer of the watcher has to do.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-06T11:26:28+01:00",
          "tree_id": "2ac37cd310eeb2573af8a46992c55bc098b36aa7",
          "url": "https://github.com/jburrow/fast_code_search/commit/ec5d6f908a26b1b90a0d0bdb9e47a497c8e80275"
        },
        "date": 1788691068251,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 323645,
            "range": "± 7923",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 30558,
            "range": "± 1021",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 416,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 321598,
            "range": "± 19837",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 30540,
            "range": "± 799",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 418,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 328563,
            "range": "± 5931",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 31179,
            "range": "± 944",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 422,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 529990,
            "range": "± 14199",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 455898,
            "range": "± 7710",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 622637,
            "range": "± 8899",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 729197,
            "range": "± 10420",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 325921,
            "range": "± 8482",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 345315,
            "range": "± 6711",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 399250,
            "range": "± 8842",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 538524,
            "range": "± 4531",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 320413,
            "range": "± 7410",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 319786,
            "range": "± 8484",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 371252,
            "range": "± 7137",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 223094,
            "range": "± 11533",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 323470,
            "range": "± 8376",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 694271,
            "range": "± 24762",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 470978,
            "range": "± 13402",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 433056,
            "range": "± 7727",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 7886,
            "range": "± 59",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 14195379,
            "range": "± 54812",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 28037764,
            "range": "± 551464",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 55687393,
            "range": "± 582050",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 25476820,
            "range": "± 89694",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 27228262,
            "range": "± 521731",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 49841386,
            "range": "± 202100",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 54010559,
            "range": "± 381600",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1172043,
            "range": "± 75242",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3563546,
            "range": "± 110475",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6646475,
            "range": "± 262073",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1237433,
            "range": "± 11265",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 4223389,
            "range": "± 28397",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 8280154,
            "range": "± 372542",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 164235,
            "range": "± 4690",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 223254,
            "range": "± 6864",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 285234,
            "range": "± 4675",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 69523,
            "range": "± 1325",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 299919,
            "range": "± 9786",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 585004,
            "range": "± 20778",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 4295267026,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 184527302,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 211279148,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 872874,
            "range": "± 54463",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 1365683,
            "range": "± 112303",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 500,
            "range": "± 71",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 1220189,
            "range": "± 78728",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 777765,
            "range": "± 183736",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 880609,
            "range": "± 85792",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 1878148,
            "range": "± 1052362",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 1666119,
            "range": "± 275398",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 1864382,
            "range": "± 600021",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 3415399,
            "range": "± 652233",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 3467632,
            "range": "± 397579",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 601944,
            "range": "± 221016",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 4908717,
            "range": "± 2698714",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 119951360,
            "range": "± 0",
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
          "id": "74df6ac917f62a0f7c9cb44c64c939308baf60b0",
          "message": "ci: show the panic message and the watcher events in test annotations\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-06T11:57:43+01:00",
          "tree_id": "341fdcff598f1c79b00034569028dc849e1cf318",
          "url": "https://github.com/jburrow/fast_code_search/commit/74df6ac917f62a0f7c9cb44c64c939308baf60b0"
        },
        "date": 1788692946255,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 326473,
            "range": "± 5850",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 30793,
            "range": "± 1576",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 416,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 330663,
            "range": "± 16744",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 31338,
            "range": "± 1055",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 416,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 330107,
            "range": "± 5446",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 31254,
            "range": "± 952",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 414,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 534953,
            "range": "± 18085",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 465635,
            "range": "± 6935",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 627365,
            "range": "± 10120",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 753292,
            "range": "± 12413",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 326383,
            "range": "± 14276",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 347196,
            "range": "± 3577",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 405488,
            "range": "± 3764",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 539130,
            "range": "± 5323",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 320549,
            "range": "± 7242",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 323348,
            "range": "± 6778",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 377073,
            "range": "± 7589",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 227641,
            "range": "± 4445",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 333368,
            "range": "± 9913",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 704341,
            "range": "± 33061",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 471781,
            "range": "± 11218",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 445717,
            "range": "± 11045",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 8090,
            "range": "± 28",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 14023585,
            "range": "± 38327",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 27867770,
            "range": "± 36172",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 55195898,
            "range": "± 248407",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 25161891,
            "range": "± 52078",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 26927630,
            "range": "± 307250",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 49434486,
            "range": "± 539620",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 53625969,
            "range": "± 75638",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1184064,
            "range": "± 75112",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3536724,
            "range": "± 115545",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6405594,
            "range": "± 120032",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1239564,
            "range": "± 16326",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 4203857,
            "range": "± 21367",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 7846207,
            "range": "± 39099",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 166915,
            "range": "± 4374",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 220491,
            "range": "± 5497",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 283872,
            "range": "± 9785",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 73030,
            "range": "± 1480",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 301293,
            "range": "± 3685",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 592385,
            "range": "± 10554",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 4191162866,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 186166885,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 198141780,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 781482,
            "range": "± 132207",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 1351478,
            "range": "± 79869",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 511,
            "range": "± 120",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 1199914,
            "range": "± 100639",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 845962,
            "range": "± 54192",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 894905,
            "range": "± 20246",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 1903210,
            "range": "± 569536",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 1691174,
            "range": "± 265907",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 1999641,
            "range": "± 273662",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 3236935,
            "range": "± 728473",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 3443332,
            "range": "± 208669",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 570958,
            "range": "± 70653",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 4053272,
            "range": "± 2664044",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 121184256,
            "range": "± 0",
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
          "id": "1c93e47d7b73e8c9ce9cce23c469ac0a1a97fec8",
          "message": "fix(watch): heal the index when a backend drops a delete or rename-away event\n\nFSEvents on macOS delivered only the destination half of a rename in\nthe end-to-end watcher test, so the old name stayed indexed as a zombie:\nits content could never be read again, search silently returned no hit\nfor it, and the file count was wrong. After applying a batch of changes\nthe engine now checks the direct children of every touched directory\nand drops entries whose file is gone (one stat per sibling).\napply_change is a one-element apply_changes so single events get the\nsame treatment. Regression test covers a lost rename-away and a lost\ndelete.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-06T13:37:39+01:00",
          "tree_id": "efd05ddc5218f32ecd9d0a7e18709c03453860d8",
          "url": "https://github.com/jburrow/fast_code_search/commit/1c93e47d7b73e8c9ce9cce23c469ac0a1a97fec8"
        },
        "date": 1788698954470,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 327879,
            "range": "± 37029",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 27285,
            "range": "± 721",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 425,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 320021,
            "range": "± 10016",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 27329,
            "range": "± 730",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 425,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 333594,
            "range": "± 12899",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 27641,
            "range": "± 644",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 430,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 539318,
            "range": "± 14858",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 456392,
            "range": "± 12420",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 604079,
            "range": "± 15994",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 745487,
            "range": "± 12653",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 323475,
            "range": "± 7858",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 340670,
            "range": "± 4218",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 393597,
            "range": "± 7652",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 542062,
            "range": "± 6518",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 318036,
            "range": "± 7510",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 320713,
            "range": "± 8526",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 387517,
            "range": "± 14944",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 231821,
            "range": "± 5682",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 330035,
            "range": "± 8983",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 701392,
            "range": "± 28211",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 487549,
            "range": "± 19788",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 458471,
            "range": "± 24221",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 9148,
            "range": "± 27",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 13777695,
            "range": "± 24279",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 27414034,
            "range": "± 123492",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 52482181,
            "range": "± 123692",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 23938057,
            "range": "± 38290",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 25928719,
            "range": "± 143577",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 47071316,
            "range": "± 172004",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 51696121,
            "range": "± 162578",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1122162,
            "range": "± 37870",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3652389,
            "range": "± 385545",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 7259763,
            "range": "± 434821",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1293786,
            "range": "± 18404",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 4471544,
            "range": "± 54722",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 8566482,
            "range": "± 224328",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 156086,
            "range": "± 4943",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 212300,
            "range": "± 4493",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 275995,
            "range": "± 7207",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 77853,
            "range": "± 3813",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 326855,
            "range": "± 3711",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 641116,
            "range": "± 4995",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 4036058764,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 193051486,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 237241885,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 761204,
            "range": "± 166760",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 1397099,
            "range": "± 282665",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 501,
            "range": "± 70",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 1281936,
            "range": "± 59499",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 861875,
            "range": "± 56344",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 899091,
            "range": "± 31227",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 1880966,
            "range": "± 1005361",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 1722338,
            "range": "± 39550",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 2009419,
            "range": "± 439701",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 3528818,
            "range": "± 498183",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 3740030,
            "range": "± 186110",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 615194,
            "range": "± 51809",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 8257338,
            "range": "± 2914658",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 121561088,
            "range": "± 0",
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
          "id": "30bc698517fecace0e0226d6ee24c86b8ac59bfe",
          "message": "test: make the concurrent-search test wait for real overlap\n\nOn a slow runner the writer finished all 100 updates before the search\nthread had taken its first read lock, so 'search thread never got the\nlock' fired (the last macOS CI failure). The test now waits for the\nsearcher to be live, keeps updating until it has searched a few more\ntimes during the writes, and asserts on the final round it reached.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-06T13:42:07+01:00",
          "tree_id": "c0486d95a465e2e5b82348f0013f5c7b7ba0ccd8",
          "url": "https://github.com/jburrow/fast_code_search/commit/30bc698517fecace0e0226d6ee24c86b8ac59bfe"
        },
        "date": 1788699631904,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 327632,
            "range": "± 6363",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 32147,
            "range": "± 823",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 420,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 325108,
            "range": "± 7077",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 31893,
            "range": "± 790",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 419,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 326564,
            "range": "± 8822",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 32519,
            "range": "± 935",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 415,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 536910,
            "range": "± 12881",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 464979,
            "range": "± 9698",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 622016,
            "range": "± 10942",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 748475,
            "range": "± 10868",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 328971,
            "range": "± 12168",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 348923,
            "range": "± 4887",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 408466,
            "range": "± 6558",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 549702,
            "range": "± 4256",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 327814,
            "range": "± 8465",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 323840,
            "range": "± 6266",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 380029,
            "range": "± 9209",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 230267,
            "range": "± 4563",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 325623,
            "range": "± 8063",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 682349,
            "range": "± 24463",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 473529,
            "range": "± 11958",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 445243,
            "range": "± 8969",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 8122,
            "range": "± 23",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 13996024,
            "range": "± 29216",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 27771518,
            "range": "± 47733",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 55249124,
            "range": "± 443804",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 25175292,
            "range": "± 47519",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 27048866,
            "range": "± 52286",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 49556029,
            "range": "± 69863",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 53873000,
            "range": "± 96778",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1209726,
            "range": "± 82052",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3551646,
            "range": "± 175727",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6448967,
            "range": "± 273481",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1239218,
            "range": "± 7658",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 4215280,
            "range": "± 20634",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 7862367,
            "range": "± 49629",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 161929,
            "range": "± 4706",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 217651,
            "range": "± 6076",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 275862,
            "range": "± 6776",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 73208,
            "range": "± 1853",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 303914,
            "range": "± 2202",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 594186,
            "range": "± 26657",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 4141915257,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 174037715,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 193886069,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 875836,
            "range": "± 69058",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 1310227,
            "range": "± 69699",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 491,
            "range": "± 150",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 1169734,
            "range": "± 95979",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 857050,
            "range": "± 51927",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 897645,
            "range": "± 46849",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 1965359,
            "range": "± 585664",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 1590429,
            "range": "± 363909",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 1983623,
            "range": "± 401690",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 3111088,
            "range": "± 554986",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 3357718,
            "range": "± 264604",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 589571,
            "range": "± 62306",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 6062728,
            "range": "± 2669975",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 122920960,
            "range": "± 0",
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
          "id": "0f82ba06e1f7092cff3b916cf5c18ccffe400361",
          "message": "release: 0.10.1\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-06T16:25:01+01:00",
          "tree_id": "9f693f534032d2ed3fda1531272d25909a92e50c",
          "url": "https://github.com/jburrow/fast_code_search/commit/0f82ba06e1f7092cff3b916cf5c18ccffe400361"
        },
        "date": 1788708969992,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 324457,
            "range": "± 7678",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 30660,
            "range": "± 984",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 423,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 324046,
            "range": "± 6656",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 30841,
            "range": "± 654",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 416,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 329499,
            "range": "± 7744",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 30032,
            "range": "± 677",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 420,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 526559,
            "range": "± 8558",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 455129,
            "range": "± 11799",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 625056,
            "range": "± 14084",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 746778,
            "range": "± 14310",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 323926,
            "range": "± 6090",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 344809,
            "range": "± 6343",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 407129,
            "range": "± 5669",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 547057,
            "range": "± 9772",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 324372,
            "range": "± 5451",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 324854,
            "range": "± 8077",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 378448,
            "range": "± 10024",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 229416,
            "range": "± 5286",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 325119,
            "range": "± 7540",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 705525,
            "range": "± 27452",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 482189,
            "range": "± 28007",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 442551,
            "range": "± 12848",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 8165,
            "range": "± 40",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 14046133,
            "range": "± 53927",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 27848129,
            "range": "± 78831",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 55210507,
            "range": "± 56541",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 25160497,
            "range": "± 46575",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 27113349,
            "range": "± 264924",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 49479998,
            "range": "± 134090",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 54211139,
            "range": "± 111797",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1154001,
            "range": "± 52594",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3481848,
            "range": "± 43824",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6365444,
            "range": "± 78876",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1216759,
            "range": "± 8601",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 4170579,
            "range": "± 18497",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 7792144,
            "range": "± 75925",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 161284,
            "range": "± 4932",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 222385,
            "range": "± 6316",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 281046,
            "range": "± 4517",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 71809,
            "range": "± 1782",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 306776,
            "range": "± 3195",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 599573,
            "range": "± 8691",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 4119474321,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 171255680,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 197404191,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 818607,
            "range": "± 124913",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 1304854,
            "range": "± 69110",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 491,
            "range": "± 100",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 1198777,
            "range": "± 74859",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 852720,
            "range": "± 38803",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 886564,
            "range": "± 46828",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 1901427,
            "range": "± 538455",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 1554440,
            "range": "± 297605",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 1973191,
            "range": "± 265044",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 3109250,
            "range": "± 283460",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 3403178,
            "range": "± 141379",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 608315,
            "range": "± 219048",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 6124425,
            "range": "± 2510985",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 122781696,
            "range": "± 0",
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
          "id": "77ef660420e7f34fcdc7ed12ea7974fbc8a557b5",
          "message": "docs: keyword engine and web UI review (2026-09-06)\n\nConsolidated findings from a code review of the trigram/query core,\nindexing lifecycle, REST/gRPC layer and keyword web UI, verified against\nthe live v0.10.1 server. Includes a prioritised list and suggested order\nof work.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-06T16:59:27+01:00",
          "tree_id": "29fccb3f0a42b29e70b62e835fa68b6467d66c17",
          "url": "https://github.com/jburrow/fast_code_search/commit/77ef660420e7f34fcdc7ed12ea7974fbc8a557b5"
        },
        "date": 1788711758610,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 325797,
            "range": "± 13969",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 29617,
            "range": "± 1402",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 414,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 317230,
            "range": "± 11753",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 29757,
            "range": "± 1860",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 420,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 321840,
            "range": "± 10534",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 29361,
            "range": "± 1083",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 413,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 548931,
            "range": "± 18971",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 457725,
            "range": "± 21155",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 616788,
            "range": "± 21742",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 763058,
            "range": "± 73154",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 311310,
            "range": "± 10754",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 359266,
            "range": "± 22164",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 400720,
            "range": "± 17347",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 552374,
            "range": "± 16105",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 313993,
            "range": "± 9722",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 307331,
            "range": "± 9885",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 373832,
            "range": "± 28719",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 221277,
            "range": "± 15676",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 316150,
            "range": "± 53509",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 710897,
            "range": "± 33647",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 494798,
            "range": "± 75247",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 436384,
            "range": "± 33760",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 7892,
            "range": "± 30",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 14021433,
            "range": "± 99785",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 27779962,
            "range": "± 90248",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 54759713,
            "range": "± 141198",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 25203849,
            "range": "± 401913",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 27609425,
            "range": "± 516369",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 50188462,
            "range": "± 773062",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 54687679,
            "range": "± 822330",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1390740,
            "range": "± 140613",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3698573,
            "range": "± 200019",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6551633,
            "range": "± 368123",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1267630,
            "range": "± 43355",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 4230128,
            "range": "± 127998",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 7947926,
            "range": "± 411871",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 157742,
            "range": "± 6070",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 218544,
            "range": "± 7517",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 288947,
            "range": "± 38049",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 72117,
            "range": "± 1498",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 299539,
            "range": "± 5760",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 583524,
            "range": "± 8148",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 4224442849,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 184083422,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 203943094,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 858864,
            "range": "± 824710",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 1360097,
            "range": "± 258386",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 512,
            "range": "± 100",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 1195939,
            "range": "± 61896",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 854656,
            "range": "± 124695",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 899691,
            "range": "± 94117",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 1904099,
            "range": "± 996704",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 1735382,
            "range": "± 642468",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 2000029,
            "range": "± 715576",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 3216085,
            "range": "± 351632",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 3682462,
            "range": "± 2300798",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 596932,
            "range": "± 241163",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 8251210,
            "range": "± 2473910",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 120758272,
            "range": "± 0",
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
          "id": "3c84b7e7f4ef268bff89562119bfb85d6a042dcd",
          "message": "fix(persist): save after symbols were re-extracted on load\n\nOtherwise the index reports itself unchanged, the save is skipped, and\nevery start pays the re-extraction again.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-07T09:38:35+01:00",
          "tree_id": "93f3c0e5ea7c060ad9211c24f6e2c64191a0a261",
          "url": "https://github.com/jburrow/fast_code_search/commit/3c84b7e7f4ef268bff89562119bfb85d6a042dcd"
        },
        "date": 1788771015758,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 312927,
            "range": "± 10634",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 20170,
            "range": "± 773",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 366,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 314602,
            "range": "± 9043",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 20467,
            "range": "± 633",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 366,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 319562,
            "range": "± 9192",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 20837,
            "range": "± 1052",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 366,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 409551,
            "range": "± 12807",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 411066,
            "range": "± 10012",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 456631,
            "range": "± 8689",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 606412,
            "range": "± 14538",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 316765,
            "range": "± 9531",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 303957,
            "range": "± 4110",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 378761,
            "range": "± 3994",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 500038,
            "range": "± 2068",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 319524,
            "range": "± 7769",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 316482,
            "range": "± 9519",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 314653,
            "range": "± 11611",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 212020,
            "range": "± 5660",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 316777,
            "range": "± 11817",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 702102,
            "range": "± 28962",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 444596,
            "range": "± 10010",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 393827,
            "range": "± 8566",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 5175,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 13119163,
            "range": "± 23781",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 26131140,
            "range": "± 78829",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 51699141,
            "range": "± 133482",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 23436525,
            "range": "± 100416",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 25191390,
            "range": "± 167698",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 46395582,
            "range": "± 303499",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 50082312,
            "range": "± 344401",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1065506,
            "range": "± 40734",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 3578566,
            "range": "± 70813",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 6669825,
            "range": "± 186155",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1194368,
            "range": "± 16630",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 4354379,
            "range": "± 60781",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 8383567,
            "range": "± 116116",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 169139,
            "range": "± 3139",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 232440,
            "range": "± 5169",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 299648,
            "range": "± 3600",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 46383,
            "range": "± 771",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 193545,
            "range": "± 2756",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 373599,
            "range": "± 9073",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 4290181217,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 150501086,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 215623004,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 762666,
            "range": "± 119610",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 1354452,
            "range": "± 100062",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 466,
            "range": "± 137",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 1030874,
            "range": "± 87487",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 682710,
            "range": "± 197477",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 894447,
            "range": "± 58472",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 1073821,
            "range": "± 180403",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 617772,
            "range": "± 148528",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 1314130,
            "range": "± 399361",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 3147283,
            "range": "± 237053",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 3417355,
            "range": "± 221693",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 562017,
            "range": "± 247605",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 6765722,
            "range": "± 3216603",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 119361536,
            "range": "± 0",
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
          "id": "8ee3909647d107dcdcb25b3c7344ddd6fd602357",
          "message": "feat(cli): fcs, a command-line client for the search server\n\nfcs sends queries to the running server over the REST API with the web\nUI's syntax and modes (search, refs, symbols; regex, globs, context,\npaging, case/word), prints path:line:col:text when piped or grouped colour\noutput on a terminal, --json for tools, and uses grep's exit codes\n(0 matches, 1 none, 2 error). When no server answers and an index_path is\nknown it loads the on-disk index in-process and runs the same engine calls\nread-only, saying so on stderr; --offline forces that, --no-offline\nforbids it. fcs status reports the server and what it holds. The server\nis found through --server, $FCS_SERVER, the configuration's web_address\nor the default; a bare query is the search subcommand and a subcommand\nnamed after flags is accepted.\n\nThe API's result types are public and deserializable and the per-match\nJSON mapping is shared with the offline path. reqwest becomes a regular\ndependency. Release archives include fcs, the startup unit files and the\nguides.\n\ndocs: docs/CLI.md (design and reference), docs/RUN-AT-STARTUP.md (start\nthe server at login on Linux/systemd, macOS/launchd, Windows/Task\nScheduler) with unit files under deploy/, README and web docs updated.\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>",
          "timestamp": "2026-09-07T09:58:52+01:00",
          "tree_id": "a0d22a897ad5cb58bcb2c53d77b64d381e230b5e",
          "url": "https://github.com/jburrow/fast_code_search/commit/8ee3909647d107dcdcb25b3c7344ddd6fd602357"
        },
        "date": 1788772243414,
        "tool": "cargo",
        "benches": [
          {
            "name": "text_search/common_query/50",
            "value": 253622,
            "range": "± 8280",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/50",
            "value": 21470,
            "range": "± 730",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/50",
            "value": 330,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/100",
            "value": 250786,
            "range": "± 8057",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/100",
            "value": 21895,
            "range": "± 516",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/100",
            "value": 350,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/common_query/200",
            "value": 248769,
            "range": "± 5289",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/rare_query/200",
            "value": 21191,
            "range": "± 377",
            "unit": "ns/iter"
          },
          {
            "name": "text_search/no_match/200",
            "value": 335,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/simple_literal",
            "value": 362644,
            "range": "± 13699",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/alternation",
            "value": 326118,
            "range": "± 7793",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/char_class",
            "value": 394578,
            "range": "± 18514",
            "unit": "ns/iter"
          },
          {
            "name": "regex_search/no_literal",
            "value": 509069,
            "range": "± 17359",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/no_filter",
            "value": 248158,
            "range": "± 7599",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_filter",
            "value": 260082,
            "range": "± 3073",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/exclude_filter",
            "value": 305886,
            "range": "± 5807",
            "unit": "ns/iter"
          },
          {
            "name": "filtered_search/include_and_exclude",
            "value": 417626,
            "range": "± 4510",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/lowercase",
            "value": 248981,
            "range": "± 8114",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/uppercase",
            "value": 245665,
            "range": "± 10902",
            "unit": "ns/iter"
          },
          {
            "name": "case_sensitivity/mixed_case",
            "value": 309636,
            "range": "± 14388",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/10",
            "value": 178003,
            "range": "± 6318",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/100",
            "value": 248831,
            "range": "± 6021",
            "unit": "ns/iter"
          },
          {
            "name": "result_limits/limit/500",
            "value": 535784,
            "range": "± 13688",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/short_2",
            "value": 369607,
            "range": "± 10489",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/medium_8",
            "value": 343159,
            "range": "± 10172",
            "unit": "ns/iter"
          },
          {
            "name": "query_length/long_16",
            "value": 7081,
            "range": "± 121",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/25",
            "value": 10480073,
            "range": "± 70464",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/50",
            "value": 20793886,
            "range": "± 44944",
            "unit": "ns/iter"
          },
          {
            "name": "indexing/index_files/100",
            "value": 41446252,
            "range": "± 116894",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/50",
            "value": 18965950,
            "range": "± 24827",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/50",
            "value": 20441636,
            "range": "± 63483",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/batch_resolve/100",
            "value": 37318938,
            "range": "± 93199",
            "unit": "ns/iter"
          },
          {
            "name": "import_resolution/incremental_every_10/100",
            "value": 40705745,
            "range": "± 275115",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/100",
            "value": 1187128,
            "range": "± 1088515",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/500",
            "value": 4200445,
            "range": "± 3312011",
            "unit": "ns/iter"
          },
          {
            "name": "index_save/1000",
            "value": 10034517,
            "range": "± 11032212",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/100",
            "value": 1059878,
            "range": "± 18062",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/500",
            "value": 3747433,
            "range": "± 17523",
            "unit": "ns/iter"
          },
          {
            "name": "index_load/1000",
            "value": 7139323,
            "range": "± 95343",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/100",
            "value": 121804,
            "range": "± 2757",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/500",
            "value": 165977,
            "range": "± 4286",
            "unit": "ns/iter"
          },
          {
            "name": "trigram_deserialization/1000",
            "value": 217133,
            "range": "± 5983",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/100",
            "value": 59441,
            "range": "± 4779",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/500",
            "value": 253057,
            "range": "± 3095",
            "unit": "ns/iter"
          },
          {
            "name": "file_staleness_check/1000",
            "value": 496237,
            "range": "± 2605",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_build",
            "value": 3070729733,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_save",
            "value": 248713003,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/index_load",
            "value": 187737891,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/common",
            "value": 646606,
            "range": "± 81182",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/identifier",
            "value": 1053629,
            "range": "± 88555",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/no_match",
            "value": 391,
            "range": "± 79",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/short",
            "value": 857845,
            "range": "± 148394",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/full_rank",
            "value": 658984,
            "range": "± 42084",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/text/filtered",
            "value": 687798,
            "range": "± 25599",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/literal",
            "value": 958446,
            "range": "± 123326",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/case_insensitive",
            "value": 496259,
            "range": "± 124432",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/regex/no_literal",
            "value": 1218269,
            "range": "± 347303",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/exact",
            "value": 2472901,
            "range": "± 305631",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/prefix",
            "value": 2631541,
            "range": "± 257878",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/symbol/references",
            "value": 475818,
            "range": "± 192741",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/incremental/modify_file",
            "value": 5835545,
            "range": "± 3971942",
            "unit": "ns/iter"
          },
          {
            "name": "corpus_bench/tokio+django/rss_after_build_bytes",
            "value": 121683968,
            "range": "± 0",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}