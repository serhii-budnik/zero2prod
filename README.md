current page: 391

11

finish:
- Add a send anewsletter issue link to the admin dashboard
- add an html form at get /admin/newsletters to submit a new issue
* adapt POST /newsletters to process the form data
    - change the route POST /admin/newsletters
    - migrate from "basic" to session-based auth
    * use the from extractor (application/x-www-form-urlencded) instead of the json extractor (application/json) to handle the request body
    * adapt the test suite
        * remove duplication for newsletters test suite
