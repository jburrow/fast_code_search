window.BENCHMARK_DATA = {
  "lastUpdate": 1772438881847,
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
      }
    ]
  }
}